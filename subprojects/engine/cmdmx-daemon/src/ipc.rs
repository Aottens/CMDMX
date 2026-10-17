//! IPC v0: WebSocket server, JSON messages and per-tick deltas (IPC-01..07), plus the
//! programmer snapshot, deltas and commands (IPC-10..13, VS2-07).
//!
//! One thread per client. The tick only talks to clients through channels and
//! never blocks: commands arrive on an unbounded channel, outgoing messages
//! leave through a bounded queue per client (IPC-07).

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender, TryRecvError};
use std::time::Duration;

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::colour::ColourModel;
use cmdmx_core::engine::Engine;
use cmdmx_core::patch::FixtureId;
use cmdmx_core::playback::PlaybackId;
use cmdmx_core::programmer::{GroupId, ProgrammerView, Resolution};
use serde::Serialize;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::http::StatusCode;
use tungstenite::{Error as WsError, Message};

/// Messages a client may have queued before it is disconnected (IPC-07).
const QUEUE: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    FaderSet {
        playback: PlaybackId,
        level: f32,
    },
    GrandMasterSet {
        level: f32,
    },
    /// IPC-13: the programmer commands map 1:1 onto engine methods.
    Select {
        fixture: FixtureId,
    },
    SelectGroup {
        group: GroupId,
    },
    Step {
        forward: bool,
    },
    Pattern {
        stages: usize,
    },
    All,
    Clear,
    AttributeGroup {
        group: AttributeGroup,
    },
    Wheel {
        attribute: String,
        steps: i32,
        resolution: Resolution,
    },
    /// IPC-18
    ColourModel {
        model: ColourModel,
    },
    Line {
        tokens: Vec<String>,
    },
    RecordGroup {
        group: GroupId,
    },
}

pub enum Event {
    Connected(SyncSender<String>),
    Command(Command),
}

/// An unsigned integer field that fits `T`.
fn uint<T: TryFrom<u64>>(v: &serde_json::Value, key: &str) -> Result<T, String> {
    v.get(key)
        .and_then(|n| n.as_u64())
        .and_then(|n| T::try_from(n).ok())
        .ok_or_else(|| format!("missing or invalid \"{key}\""))
}

/// IPC-04/06/13: one JSON text frame to a command, or a human-readable error.
pub fn parse_command(text: &str) -> Result<Command, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("bad JSON: {e}"))?;
    if v.get("v").and_then(|n| n.as_u64()) != Some(0) {
        return Err("missing or unsupported \"v\" (expected 0)".into());
    }
    let level = || {
        v.get("level")
            .and_then(|n| n.as_f64())
            .map(|n| (n as f32).clamp(0.0, 1.0))
            .ok_or_else(|| "missing or invalid \"level\"".to_string())
    };
    match v.get("cmd").and_then(|c| c.as_str()) {
        Some("fader.set") => {
            let playback = v
                .get("playback")
                .and_then(|n| n.as_u64())
                .and_then(|n| PlaybackId::try_from(n).ok())
                .ok_or("missing or invalid \"playback\"")?;
            Ok(Command::FaderSet {
                playback,
                level: level()?,
            })
        }
        Some("grand_master.set") => Ok(Command::GrandMasterSet { level: level()? }),
        Some("programmer.select") => Ok(Command::Select {
            fixture: uint(&v, "fixture")?,
        }),
        Some("programmer.select_group") => Ok(Command::SelectGroup {
            group: uint(&v, "group")?,
        }),
        Some("programmer.step") => Ok(Command::Step {
            forward: match v.get("direction").and_then(|n| n.as_i64()) {
                Some(1) => true,
                Some(-1) => false,
                _ => return Err("missing or invalid \"direction\" (expected 1 or -1)".into()),
            },
        }),
        Some("programmer.pattern") => Ok(Command::Pattern {
            stages: uint(&v, "stages")?,
        }),
        Some("programmer.all") => Ok(Command::All),
        Some("programmer.clear") => Ok(Command::Clear),
        Some("programmer.attribute_group") => Ok(Command::AttributeGroup {
            group: v
                .get("group")
                .and_then(|g| serde_json::from_value(g.clone()).ok())
                .ok_or("missing or invalid \"group\" (expected I P C G B E S FX)")?,
        }),
        Some("programmer.colour_model") => Ok(Command::ColourModel {
            model: v
                .get("model")
                .and_then(|m| serde_json::from_value(m.clone()).ok())
                .ok_or("missing or invalid \"model\" (expected Hsi, Rgb or Cmy)")?,
        }),
        Some("programmer.wheel") => Ok(Command::Wheel {
            attribute: v
                .get("attribute")
                .and_then(|a| a.as_str())
                .ok_or("missing or invalid \"attribute\"")?
                .to_string(),
            steps: v
                .get("steps")
                .and_then(|n| n.as_i64())
                .and_then(|n| i32::try_from(n).ok())
                .ok_or("missing or invalid \"steps\"")?,
            resolution: match v.get("resolution") {
                None => Resolution::Normal,
                Some(r) => serde_json::from_value(r.clone())
                    .map_err(|_| "invalid \"resolution\" (expected Normal, Fine or Coarse)")?,
            },
        }),
        Some("programmer.command") => Ok(Command::Line {
            tokens: v
                .get("tokens")
                .and_then(|t| t.as_array())
                .and_then(|a| a.iter().map(|t| t.as_str().map(String::from)).collect())
                .ok_or("missing or invalid \"tokens\" (expected an array of strings)")?,
        }),
        Some("group.record") => Ok(Command::RecordGroup {
            group: uint(&v, "group")?,
        }),
        Some(other) => Err(format!("unknown cmd '{other}'")),
        None => Err("missing \"cmd\"".into()),
    }
}

#[derive(Serialize)]
struct PlaybackInfo {
    id: PlaybackId,
    level: f32,
    active: bool,
}

#[derive(Serialize)]
struct FixtureInfo<'a> {
    id: FixtureId,
    profile: &'a str,
}

#[derive(Serialize)]
struct GroupInfo<'a> {
    id: GroupId,
    fixtures: &'a [FixtureId],
}

#[derive(Serialize)]
struct Snapshot<'a> {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    faders_per_page: u16,
    grand_master: f32,
    playbacks: Vec<PlaybackInfo>,
    fixtures: Vec<FixtureInfo<'a>>,
    groups: Vec<GroupInfo<'a>>,
    programmer: &'a ProgrammerView,
}

#[derive(Serialize)]
struct ProgrammerDelta<'a> {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    view: &'a ProgrammerView,
}

#[derive(Serialize)]
struct GroupsDelta<'a> {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    groups: Vec<GroupInfo<'a>>,
}

#[derive(Serialize)]
struct PlaybackDelta {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    info: PlaybackInfo,
}

#[derive(Serialize)]
struct GrandMasterDelta {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    level: f32,
}

#[derive(Serialize)]
struct Text<'a> {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    message: &'a str,
}

/// Serialisation of these plain structs cannot fail; if it ever did, an empty
/// string is skipped by the clients rather than panicking.
fn to_json(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn infos(engine: &Engine) -> impl Iterator<Item = PlaybackInfo> + '_ {
    engine.playback_states().map(|(id, s)| PlaybackInfo {
        id: *id,
        level: s.level,
        active: s.activated.is_some(),
    })
}

fn group_infos(groups: &BTreeMap<GroupId, Vec<FixtureId>>) -> Vec<GroupInfo<'_>> {
    groups
        .iter()
        .map(|(id, fixtures)| GroupInfo { id: *id, fixtures })
        .collect()
}

/// IPC-03/10: full state for a freshly connected client.
pub fn snapshot(engine: &Engine) -> String {
    to_json(&Snapshot {
        v: 0,
        kind: "snapshot",
        faders_per_page: engine.settings().faders_per_page,
        grand_master: engine.grand_master(),
        playbacks: infos(engine).collect(),
        fixtures: engine
            .fixtures()
            .map(|f| FixtureInfo {
                id: f.id,
                profile: &f.profile,
            })
            .collect(),
        groups: group_infos(engine.groups()),
        programmer: &engine.programmer_view(),
    })
}

pub fn error(msg: &str) -> String {
    to_json(&Text {
        v: 0,
        kind: "error",
        message: msg,
    })
}

pub fn warning(msg: &str) -> String {
    to_json(&Text {
        v: 0,
        kind: "warning",
        message: msg,
    })
}

/// What clients have last been told, to emit deltas only for changes (IPC-05, IPC-12).
pub struct Published {
    playbacks: BTreeMap<PlaybackId, (f32, bool)>,
    grand_master: f32,
    programmer: ProgrammerView,
    groups: BTreeMap<GroupId, Vec<FixtureId>>,
}

impl Published {
    pub fn new(engine: &Engine) -> Self {
        Self {
            playbacks: infos(engine).map(|i| (i.id, (i.level, i.active))).collect(),
            grand_master: engine.grand_master(),
            programmer: engine.programmer_view(),
            groups: engine.groups().clone(),
        }
    }

    pub fn diff(&mut self, engine: &Engine) -> Vec<String> {
        let mut out = Vec::new();
        for info in infos(engine) {
            let now = (info.level, info.active);
            if self.playbacks.insert(info.id, now) != Some(now) {
                out.push(to_json(&PlaybackDelta {
                    v: 0,
                    kind: "playback",
                    info,
                }));
            }
        }
        let gm = engine.grand_master();
        if gm != self.grand_master {
            self.grand_master = gm;
            out.push(to_json(&GrandMasterDelta {
                v: 0,
                kind: "grand_master",
                level: gm,
            }));
        }
        // IPC-12: at most one programmer and one groups delta per tick.
        let view = engine.programmer_view();
        if view != self.programmer {
            out.push(to_json(&ProgrammerDelta {
                v: 0,
                kind: "programmer",
                view: &view,
            }));
            self.programmer = view;
        }
        if engine.groups() != &self.groups {
            self.groups = engine.groups().clone();
            out.push(to_json(&GroupsDelta {
                v: 0,
                kind: "groups",
                groups: group_infos(&self.groups),
            }));
        }
        out
    }
}

/// Outgoing queues of all connected clients, owned by the tick thread.
#[derive(Default)]
pub struct Clients(Vec<SyncSender<String>>);

impl Clients {
    /// Queue the snapshot first; a client that cannot even take that is not kept.
    pub fn add(&mut self, tx: SyncSender<String>, snapshot: &str) {
        if tx.try_send(snapshot.to_string()).is_ok() {
            self.0.push(tx);
        }
    }

    /// Never blocks: a client whose queue is full or closed is dropped (IPC-07).
    pub fn broadcast(&mut self, msg: &str) {
        self.0.retain(|tx| tx.try_send(msg.to_string()).is_ok());
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// IPC-01: listen on loopback (port 0 = ephemeral); events flow to the tick.
pub fn spawn_server(port: u16) -> std::io::Result<(Receiver<Event>, SocketAddr)> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
    let addr = listener.local_addr()?;
    let (events, rx) = channel();
    std::thread::Builder::new()
        .name("ipc-listener".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let events = events.clone();
                // A failed spawn only loses this one client.
                let _ = std::thread::Builder::new()
                    .name("ipc-client".into())
                    .spawn(move || {
                        // IPC-07: a panic closes only this client.
                        if catch_unwind(AssertUnwindSafe(|| serve_client(stream, &events))).is_err()
                        {
                            eprintln!("ipc: client thread panicked; connection closed");
                        }
                    });
            }
        })?;
    Ok((rx, addr))
}

/// IPC-02: browsers always send an Origin header; refuse those.
#[allow(clippy::result_large_err)] // the error type is dictated by tungstenite's callback trait
fn refuse_origin(req: &Request, resp: Response) -> Result<Response, ErrorResponse> {
    if req.headers().contains_key("origin") {
        let mut refusal = ErrorResponse::new(Some("browser origins are not allowed".into()));
        *refusal.status_mut() = StatusCode::FORBIDDEN;
        return Err(refusal);
    }
    Ok(resp)
}

fn serve_client(stream: TcpStream, events: &Sender<Event>) {
    let Ok(mut ws) = tungstenite::accept_hdr(stream, refuse_origin) else {
        return;
    };
    let (tx, out) = sync_channel::<String>(QUEUE);
    if events.send(Event::Connected(tx)).is_err() {
        return;
    }
    // After the handshake: short timeouts so one thread can both read and write.
    let tcp = ws.get_mut();
    if tcp
        .set_read_timeout(Some(Duration::from_millis(20)))
        .is_err()
        || tcp.set_write_timeout(Some(Duration::from_secs(2))).is_err()
    {
        return;
    }
    loop {
        match ws.read() {
            Ok(Message::Text(text)) => match parse_command(text.as_str()) {
                Ok(cmd) => {
                    if events.send(Event::Command(cmd)).is_err() {
                        return;
                    }
                }
                Err(e) => {
                    if ws.send(Message::text(error(&e))).is_err() {
                        return;
                    }
                }
            },
            Ok(Message::Close(_)) => return,
            Ok(_) => {}
            Err(WsError::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return,
        }
        loop {
            match out.try_recv() {
                Ok(msg) => {
                    if ws.send(Message::text(msg)).is_err() {
                        return;
                    }
                }
                Err(TryRecvError::Empty) => break,
                // The tick dropped this client (queue full, IPC-07) or is gone.
                Err(TryRecvError::Disconnected) => return,
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]
mod tests {
    use super::*;
    use cmdmx_core::attribute::{AttributeDef, AttributeGroup, AttributeKind};
    use cmdmx_core::colour::ColourModel;
    use cmdmx_core::engine::{Engine, Settings};
    use cmdmx_core::patch::Fixture;
    use cmdmx_core::playback::{FaderMode, Playback, Priority};
    use cmdmx_core::profile::Profile;
    use cmdmx_core::programmer::Resolution;
    use serde_json::{json, Value};
    use std::collections::BTreeMap;
    use std::net::TcpStream;
    use std::sync::mpsc::sync_channel;
    use std::time::Duration;
    use tungstenite::client::IntoClientRequest;
    use tungstenite::Message;

    fn engine() -> Engine {
        let mut e = Engine::new(Settings::default());
        e.add_profile(
            "dimmer",
            Profile::from_attributes(vec![AttributeDef::new(
                "Dimmer",
                AttributeGroup::I,
                AttributeKind::Htp,
                1,
            )]),
        )
        .unwrap();
        e.patch(Fixture {
            id: 1,
            number: 0,
            name: String::new(),
            handle: None,
            profile: "dimmer".into(),
            mode: None,
            universe: 1,
            address: 1,
        })
        .unwrap();
        e.add_playback(Playback {
            id: 1,
            priority: Priority::Normal,
            fader_mode: FaderMode::Mode0,
            values: BTreeMap::from([(1, BTreeMap::from([("Dimmer".to_string(), 1.0.into())]))]),
        });
        e
    }

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn ipc_03_snapshot_has_settings_grand_master_and_playbacks() {
        let v = parse(&snapshot(&engine()));
        assert_eq!(v["v"], 0);
        assert_eq!(v["type"], "snapshot");
        assert_eq!(v["faders_per_page"], 8);
        assert_eq!(v["grand_master"], 1.0);
        assert_eq!(v["playbacks"], json!([{"id":1,"level":0.0,"active":false}]));
    }

    fn no_values(available_i: bool) -> Value {
        let groups = ["I", "P", "C", "G", "B", "E", "S", "FX"];
        Value::Array(
            groups
                .iter()
                .map(|g| json!({"group": g, "available": available_i && *g == "I", "has_values": false}))
                .collect(),
        )
    }

    #[test]
    fn ipc_10_snapshot_has_fixtures_groups_and_programmer() {
        let mut e = engine();
        e.select_fixture(1).unwrap();
        e.record_group(1).unwrap();
        e.clear();
        let v = parse(&snapshot(&e));
        assert_eq!(v["fixtures"], json!([{"id":1,"profile":"dimmer"}]));
        assert_eq!(v["groups"], json!([{"id":1,"fixtures":[1]}]));
        assert_eq!(
            v["programmer"],
            json!({"selection":[],"targets":[],"active_group":"I",
                   "attribute_groups": no_values(false),"wheels":[],"in_programmer":[],"colour_model":"Hsi"})
        );
    }

    #[test]
    fn ipc_11_programmer_state_object() {
        let mut e = engine();
        e.select_fixture(1).unwrap();
        let v = parse(&snapshot(&e));
        assert_eq!(
            v["programmer"],
            json!({"selection":[1],"targets":[1],"active_group":"I",
                   "attribute_groups": no_values(true),
                   "wheels":[{"attribute":"Dimmer","value":0.0,"touched":false,"sixteen_bit":false}],
                   "in_programmer":[],"colour_model":"Hsi"})
        );
        e.turn_wheel("Dimmer", 255, Resolution::Normal).unwrap();
        let v = parse(&snapshot(&e));
        assert_eq!(
            v["programmer"]["wheels"],
            json!([{"attribute":"Dimmer","value":1.0,"touched":true,"sixteen_bit":false}])
        );
        assert_eq!(v["programmer"]["in_programmer"], json!([1]));
        assert_eq!(v["programmer"]["attribute_groups"][0]["has_values"], true);
    }

    #[test]
    fn ipc_12_programmer_and_groups_deltas_only_on_change() {
        let mut e = engine();
        let mut published = Published::new(&e);
        e.tick(0.0);
        assert!(published.diff(&e).is_empty());

        e.select_fixture(1).unwrap();
        e.tick(0.025);
        let d = published.diff(&e);
        assert_eq!(d.len(), 1);
        let p = parse(&d[0]);
        assert_eq!(p["v"], 0);
        assert_eq!(p["type"], "programmer");
        assert_eq!(p["selection"], json!([1]));
        assert_eq!(p["wheels"][0]["attribute"], "Dimmer");

        e.tick(0.05);
        assert!(published.diff(&e).is_empty(), "no change, no delta");

        e.record_group(4).unwrap();
        e.tick(0.075);
        let d = published.diff(&e);
        assert_eq!(d.len(), 1);
        assert_eq!(
            parse(&d[0]),
            json!({"v":0,"type":"groups","groups":[{"id":4,"fixtures":[1]}]})
        );
    }

    #[test]
    fn ipc_13_programmer_commands_parse() {
        let ok = |s: &str| parse_command(s).unwrap();
        assert_eq!(
            ok(r#"{"v":0,"cmd":"programmer.select","fixture":201}"#),
            Command::Select { fixture: 201 }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"programmer.select_group","group":3}"#),
            Command::SelectGroup { group: 3 }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"programmer.step","direction":-1}"#),
            Command::Step { forward: false }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"programmer.step","direction":1}"#),
            Command::Step { forward: true }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"programmer.pattern","stages":2}"#),
            Command::Pattern { stages: 2 }
        );
        assert_eq!(ok(r#"{"v":0,"cmd":"programmer.all"}"#), Command::All);
        assert_eq!(ok(r#"{"v":0,"cmd":"programmer.clear"}"#), Command::Clear);
        assert_eq!(
            ok(r#"{"v":0,"cmd":"programmer.attribute_group","group":"P"}"#),
            Command::AttributeGroup {
                group: AttributeGroup::P
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"programmer.attribute_group","group":"FX"}"#),
            Command::AttributeGroup {
                group: AttributeGroup::Fx
            }
        );
        assert_eq!(
            ok(
                r#"{"v":0,"cmd":"programmer.wheel","attribute":"Pan","steps":-3,"resolution":"Fine"}"#
            ),
            Command::Wheel {
                attribute: "Pan".into(),
                steps: -3,
                resolution: Resolution::Fine
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"programmer.wheel","attribute":"Pan","steps":2}"#),
            Command::Wheel {
                attribute: "Pan".into(),
                steps: 2,
                resolution: Resolution::Normal
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"programmer.command","tokens":["201","Thro","205","Enter"]}"#),
            Command::Line {
                tokens: vec!["201".into(), "Thro".into(), "205".into(), "Enter".into()]
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"group.record","group":3}"#),
            Command::RecordGroup { group: 3 }
        );
    }

    #[test]
    fn ipc_18_colour_model_command_parses() {
        for (name, model) in [
            ("Hsi", ColourModel::Hsi),
            ("Rgb", ColourModel::Rgb),
            ("Cmy", ColourModel::Cmy),
        ] {
            assert_eq!(
                parse_command(&format!(
                    r#"{{"v":0,"cmd":"programmer.colour_model","model":"{name}"}}"#
                )),
                Ok(Command::ColourModel { model })
            );
        }
        for bad in [
            r#"{"v":0,"cmd":"programmer.colour_model"}"#,
            r#"{"v":0,"cmd":"programmer.colour_model","model":"Lab"}"#,
            r#"{"v":0,"cmd":"programmer.colour_model","model":2}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn ipc_18_colour_model_change_is_a_programmer_delta() {
        let mut e = engine();
        let mut published = Published::new(&e);
        e.tick(0.0);
        assert!(published.diff(&e).is_empty());
        e.set_colour_model(ColourModel::Cmy);
        e.tick(0.025);
        let d = published.diff(&e);
        assert_eq!(d.len(), 1);
        let p = parse(&d[0]);
        assert_eq!(p["type"], "programmer");
        assert_eq!(p["colour_model"], "Cmy");
    }

    #[test]
    fn ipc_13_bad_programmer_commands_are_errors() {
        for bad in [
            r#"{"v":0,"cmd":"programmer.select"}"#,
            r#"{"v":0,"cmd":"programmer.select","fixture":"201"}"#,
            r#"{"v":0,"cmd":"programmer.step","direction":2}"#,
            r#"{"v":0,"cmd":"programmer.pattern","stages":-1}"#,
            r#"{"v":0,"cmd":"programmer.attribute_group","group":"Q"}"#,
            r#"{"v":0,"cmd":"programmer.wheel","attribute":"Pan","steps":1.5}"#,
            r#"{"v":0,"cmd":"programmer.wheel","steps":1}"#,
            r#"{"v":0,"cmd":"programmer.wheel","attribute":"Pan","steps":1,"resolution":"Huge"}"#,
            r#"{"v":0,"cmd":"programmer.command","tokens":"201 Enter"}"#,
            r#"{"v":0,"cmd":"programmer.command","tokens":[201]}"#,
            r#"{"v":0,"cmd":"group.record"}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn ipc_04_commands_parse_and_levels_are_clamped() {
        assert_eq!(
            parse_command(r#"{"v":0,"cmd":"fader.set","playback":2,"level":0.5}"#),
            Ok(Command::FaderSet {
                playback: 2,
                level: 0.5
            })
        );
        assert_eq!(
            parse_command(r#"{"v":0,"cmd":"fader.set","playback":1,"level":1.7}"#),
            Ok(Command::FaderSet {
                playback: 1,
                level: 1.0
            })
        );
        assert_eq!(
            parse_command(r#"{"v":0,"cmd":"grand_master.set","level":-1}"#),
            Ok(Command::GrandMasterSet { level: 0.0 })
        );
    }

    #[test]
    fn ipc_06_bad_input_is_an_error() {
        for bad in [
            "not json",
            r#"{"v":0,"cmd":"self.destruct"}"#,
            r#"{"v":0,"cmd":"fader.set","level":0.5}"#,
            r#"{"v":1,"cmd":"grand_master.set","level":0.5}"#,
            r#"{"cmd":"grand_master.set","level":0.5}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
        let v = parse(&error("boom"));
        assert_eq!(v, json!({"v":0,"type":"error","message":"boom"}));
        assert_eq!(parse(&warning("w"))["type"], "warning");
    }

    #[test]
    fn ipc_05_deltas_only_for_changed_items() {
        let mut e = engine();
        let mut published = Published::new(&e);
        e.tick(0.0);
        assert!(published.diff(&e).is_empty());

        e.set_fader(1, 0.5, 0.025).unwrap();
        e.tick(0.025);
        let d = published.diff(&e);
        assert_eq!(d.len(), 1);
        assert_eq!(
            parse(&d[0]),
            json!({"v":0,"type":"playback","id":1,"level":0.5,"active":true})
        );
        e.tick(0.05);
        assert!(published.diff(&e).is_empty(), "no change, no delta");

        e.set_grand_master(0.25).unwrap();
        let d = published.diff(&e);
        assert_eq!(
            parse(&d[0]),
            json!({"v":0,"type":"grand_master","level":0.25})
        );
    }

    #[test]
    fn ipc_07_full_or_closed_client_is_dropped() {
        let mut clients = Clients::default();
        let (slow_tx, _slow_rx) = sync_channel(1);
        let (gone_tx, gone_rx) = sync_channel(64);
        let (ok_tx, ok_rx) = sync_channel(64);
        clients.add(slow_tx, "snap"); // fills the slow client's queue
        clients.add(gone_tx, "snap");
        clients.add(ok_tx, "snap");
        drop(gone_rx);
        clients.broadcast("delta");
        assert_eq!(clients.len(), 1);
        assert_eq!(ok_rx.try_recv().unwrap(), "snap");
        assert_eq!(ok_rx.try_recv().unwrap(), "delta");
    }

    fn connect(addr: SocketAddr) -> tungstenite::WebSocket<TcpStream> {
        let stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        tungstenite::client("ws://127.0.0.1/", stream).unwrap().0
    }

    fn next_event(rx: &Receiver<Event>) -> Event {
        rx.recv_timeout(Duration::from_secs(2)).unwrap()
    }

    #[test]
    fn ipc_01_server_sends_queued_messages_and_forwards_commands() {
        let (rx, addr) = spawn_server(0).unwrap();
        assert!(addr.ip().is_loopback());
        let mut ws = connect(addr);
        let Event::Connected(tx) = next_event(&rx) else {
            panic!("expected Connected")
        };
        tx.send(snapshot(&engine())).unwrap();
        let msg = ws.read().unwrap();
        assert_eq!(parse(msg.to_text().unwrap())["type"], "snapshot");

        ws.send(Message::text(
            r#"{"v":0,"cmd":"fader.set","playback":1,"level":0.5}"#,
        ))
        .unwrap();
        match next_event(&rx) {
            Event::Command(c) => assert_eq!(
                c,
                Command::FaderSet {
                    playback: 1,
                    level: 0.5
                }
            ),
            Event::Connected(_) => panic!("expected Command"),
        }
    }

    #[test]
    fn ipc_06_bad_message_gets_error_and_connection_stays() {
        let (rx, addr) = spawn_server(0).unwrap();
        let mut ws = connect(addr);
        let _tick_side = next_event(&rx); // keep the client's queue alive
        ws.send(Message::text("garbage")).unwrap();
        let msg = ws.read().unwrap();
        assert_eq!(parse(msg.to_text().unwrap())["type"], "error");
        ws.send(Message::text(
            r#"{"v":0,"cmd":"grand_master.set","level":0.5}"#,
        ))
        .unwrap();
        assert!(matches!(next_event(&rx), Event::Command(_)));
    }

    #[test]
    fn ipc_02_handshake_with_origin_is_refused() {
        let (rx, addr) = spawn_server(0).unwrap();
        let stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut req = "ws://127.0.0.1/".into_client_request().unwrap();
        req.headers_mut()
            .insert("Origin", "http://evil.example".parse().unwrap());
        assert!(tungstenite::client(req, stream).is_err());
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    }
}

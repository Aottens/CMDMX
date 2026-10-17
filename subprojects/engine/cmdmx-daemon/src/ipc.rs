//! IPC v0: WebSocket server, JSON messages and per-tick deltas (IPC-01..07), plus the
//! programmer snapshot, deltas and commands (IPC-10..13, VS2-07), and the fixture library
//! requests (LIB-14, VS2-11), the patch commands and `fixtures` delta (PAT-09, VS2-12), and
//! the output node states in the snapshot and the `outputs` delta (IPC-20).
//!
//! One thread per client. The tick only talks to clients through channels and
//! never blocks: commands arrive on an unbounded channel, outgoing messages
//! leave through a bounded queue per client (IPC-07). Library requests never reach the tick:
//! the client thread answers them itself, straight to the asking client.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender, TryRecvError};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::colour::ColourModel;
use cmdmx_core::engine::Engine;
use cmdmx_core::library::channel_list::ChannelList;
use cmdmx_core::patch::{Fixture, FixtureId, Handle, PatchAdd};
use cmdmx_core::playback::PlaybackId;
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::{GroupId, ProgrammerView, Resolution};
use cmdmx_core::UniverseId;
use serde::Serialize;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::http::StatusCode;
use tungstenite::{Error as WsError, Message};

use crate::library::Library;
use crate::output::OutputState;

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
    /// LIB-14: answered by the client thread, never applied by the tick.
    Library(LibraryRequest),
    /// PAT-09: `profile` is the library's copy, attached by the client thread (`attach_profile`).
    PatchAdd {
        add: PatchAdd,
        profile: Option<Profile>,
    },
    PatchMove {
        id: FixtureId,
        universe: UniverseId,
        address: u16,
    },
    PatchRemove {
        ids: Vec<FixtureId>,
    },
    PatchRenumber {
        id: FixtureId,
        number: u32,
    },
    PatchName {
        id: FixtureId,
        name: String,
    },
    PatchChangeMode {
        id: FixtureId,
        mode: String,
    },
    PatchHandle {
        id: FixtureId,
        handle: Handle,
    },
}

/// LIB-14: the fixture library requests.
#[derive(Debug, Clone, PartialEq)]
pub enum LibraryRequest {
    Search { query: String },
    ImportReport { manufacturer: String, model: String },
    Profile { manufacturer: String, model: String },
    SaveProfile(Profile),
    SaveChannelList(ChannelList),
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

/// A string field.
fn string(v: &serde_json::Value, key: &str) -> Result<String, String> {
    v.get(key)
        .and_then(|s| s.as_str())
        .map(String::from)
        .ok_or_else(|| format!("missing or invalid \"{key}\""))
}

/// An optional unsigned integer field: absent is `None`, present but invalid is an error.
fn opt_uint<T: TryFrom<u64>>(v: &serde_json::Value, key: &str) -> Result<Option<T>, String> {
    match v.get(key) {
        None => Ok(None),
        Some(_) => uint(v, key).map(Some),
    }
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
        Some("library.search") => Ok(Command::Library(LibraryRequest::Search {
            query: string(&v, "query")?,
        })),
        Some("library.import_report") => Ok(Command::Library(LibraryRequest::ImportReport {
            manufacturer: string(&v, "manufacturer")?,
            model: string(&v, "model")?,
        })),
        Some("library.profile") => Ok(Command::Library(LibraryRequest::Profile {
            manufacturer: string(&v, "manufacturer")?,
            model: string(&v, "model")?,
        })),
        Some("library.save_profile") => Ok(Command::Library(LibraryRequest::SaveProfile(
            serde_json::from_value(v.get("profile").cloned().unwrap_or_default())
                .map_err(|e| format!("missing or invalid \"profile\": {e}"))?,
        ))),
        Some("library.save_channel_list") => Ok(Command::Library(LibraryRequest::SaveChannelList(
            serde_json::from_value(v.clone()).map_err(|e| format!("invalid channel list: {e}"))?,
        ))),
        Some("patch.add") => Ok(Command::PatchAdd {
            add: PatchAdd {
                profile: string(&v, "profile")?,
                mode: Some(string(&v, "mode")?),
                count: uint(&v, "count")?,
                universe: uint(&v, "universe")?,
                address: opt_uint(&v, "address")?,
                number: opt_uint(&v, "number")?,
            },
            profile: None,
        }),
        Some("patch.move") => Ok(Command::PatchMove {
            id: uint(&v, "id")?,
            universe: uint(&v, "universe")?,
            address: uint(&v, "address")?,
        }),
        Some("patch.remove") => Ok(Command::PatchRemove {
            ids: v
                .get("ids")
                .and_then(|t| t.as_array())
                .and_then(|a| a.iter().map(|n| n.as_u64()?.try_into().ok()).collect())
                .ok_or("missing or invalid \"ids\" (expected an array of fixture ids)")?,
        }),
        Some("patch.renumber") => Ok(Command::PatchRenumber {
            id: uint(&v, "id")?,
            number: uint(&v, "number")?,
        }),
        Some("patch.name") => Ok(Command::PatchName {
            id: uint(&v, "id")?,
            name: string(&v, "name")?,
        }),
        Some("patch.change_mode") => Ok(Command::PatchChangeMode {
            id: uint(&v, "id")?,
            mode: string(&v, "mode")?,
        }),
        Some("patch.handle") => Ok(Command::PatchHandle {
            id: uint(&v, "id")?,
            handle: Handle {
                page: uint(&v, "page")?,
                slot: uint(&v, "slot")?,
            },
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
    number: u32,
    name: &'a str,
    profile: &'a str,
    mode: &'a str,
    universe: UniverseId,
    address: u16,
    footprint: u16,
    virtual_dimmer: bool,
    handle: Option<Handle>,
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
    outputs: Vec<OutputJson>,
}

/// IPC-20: one output node as the daemon's tick sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputStatus {
    pub name: &'static str,
    pub state: OutputState,
}

#[derive(Serialize)]
struct OutputJson {
    name: &'static str,
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<&'static str>,
}

impl From<&OutputStatus> for OutputJson {
    fn from(o: &OutputStatus) -> Self {
        let (state, detail) = match o.state {
            OutputState::Ok => ("Ok", None),
            OutputState::Degraded => ("Degraded", None),
            OutputState::NoInterface => ("Degraded", Some("no interface")),
            OutputState::Stopped => ("Stopped", None),
        };
        OutputJson {
            name: o.name,
            state,
            detail,
        }
    }
}

fn output_jsons(outputs: &[OutputStatus]) -> Vec<OutputJson> {
    outputs.iter().map(OutputJson::from).collect()
}

#[derive(Serialize)]
struct OutputsDelta {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    outputs: Vec<OutputJson>,
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
struct FixturesDelta<'a> {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    fixtures: Vec<FixtureInfo<'a>>,
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

/// PAT-09: the patch as clients see it, with each fixture's mode described.
fn fixture_infos(engine: &Engine) -> Vec<FixtureInfo<'_>> {
    engine
        .fixtures()
        .map(|f| {
            let mode = engine.fixture_mode(f.id);
            FixtureInfo {
                id: f.id,
                number: f.number,
                name: &f.name,
                profile: &f.profile,
                mode: mode.map_or("", |m| m.name.as_str()),
                universe: f.universe,
                address: f.address,
                footprint: mode.map_or(0, |m| m.footprint),
                virtual_dimmer: mode.is_some_and(|m| m.has_virtual_dimmer()),
                handle: f.handle,
            }
        })
        .collect()
}

/// IPC-03/10/20: full state for a freshly connected client.
pub fn snapshot(engine: &Engine, outputs: &[OutputStatus]) -> String {
    to_json(&Snapshot {
        v: 0,
        kind: "snapshot",
        faders_per_page: engine.settings().faders_per_page,
        grand_master: engine.grand_master(),
        playbacks: infos(engine).collect(),
        fixtures: fixture_infos(engine),
        groups: group_infos(engine.groups()),
        programmer: &engine.programmer_view(),
        outputs: output_jsons(outputs),
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
    fixtures: Vec<Fixture>,
    outputs: Vec<OutputStatus>,
}

impl Published {
    pub fn new(engine: &Engine, outputs: &[OutputStatus]) -> Self {
        Self {
            playbacks: infos(engine).map(|i| (i.id, (i.level, i.active))).collect(),
            grand_master: engine.grand_master(),
            programmer: engine.programmer_view(),
            groups: engine.groups().clone(),
            fixtures: engine.fixtures().cloned().collect(),
            outputs: outputs.to_vec(),
        }
    }

    pub fn diff(&mut self, engine: &Engine, outputs: &[OutputStatus]) -> Vec<String> {
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
        // PAT-09: the whole list, once, after any patch change.
        if !engine.fixtures().eq(self.fixtures.iter()) {
            self.fixtures = engine.fixtures().cloned().collect();
            out.push(to_json(&FixturesDelta {
                v: 0,
                kind: "fixtures",
                fixtures: fixture_infos(engine),
            }));
        }
        // IPC-20: the whole list, once, after any node changed state.
        if outputs != self.outputs.as_slice() {
            self.outputs = outputs.to_vec();
            out.push(to_json(&OutputsDelta {
                v: 0,
                kind: "outputs",
                outputs: output_jsons(outputs),
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

/// The library as the daemon shares it: `None` until the background load has finished.
pub type SharedLibrary = Arc<RwLock<Option<Library>>>;

const LIBRARY_LOADING: &str = "library is loading; try again in a moment";
const LIBRARY_POISONED: &str = "library is unavailable after an internal error";

fn saved(manufacturer: &str, model: &str) -> serde_json::Value {
    serde_json::json!({"v": 0, "type": "library.saved", "manufacturer": manufacturer, "model": model})
}

fn library_reply(
    lib: &RwLock<Option<Library>>,
    req: LibraryRequest,
) -> Result<serde_json::Value, String> {
    use serde_json::json;
    match req {
        LibraryRequest::SaveProfile(profile) => {
            let mut guard = lib.write().map_err(|_| LIBRARY_POISONED)?;
            guard
                .as_mut()
                .ok_or(LIBRARY_LOADING)?
                .save_profile(&profile)?;
            Ok(saved(&profile.manufacturer, &profile.model))
        }
        LibraryRequest::SaveChannelList(list) => {
            let mut guard = lib.write().map_err(|_| LIBRARY_POISONED)?;
            guard
                .as_mut()
                .ok_or(LIBRARY_LOADING)?
                .save_channel_list(&list)?;
            Ok(saved(&list.manufacturer, &list.model))
        }
        LibraryRequest::Search { query } => {
            let guard = lib.read().map_err(|_| LIBRARY_POISONED)?;
            let found = guard.as_ref().ok_or(LIBRARY_LOADING)?.search(&query);
            Ok(json!({"v": 0, "type": "library.results", "items": found}))
        }
        LibraryRequest::ImportReport {
            manufacturer,
            model,
        } => {
            let guard = lib.read().map_err(|_| LIBRARY_POISONED)?;
            let report = guard
                .as_ref()
                .ok_or(LIBRARY_LOADING)?
                .import_report(&manufacturer, &model)?;
            Ok(
                json!({"v": 0, "type": "library.import_report", "manufacturer": manufacturer,
                      "model": model, "report": report}),
            )
        }
        LibraryRequest::Profile {
            manufacturer,
            model,
        } => {
            let guard = lib.read().map_err(|_| LIBRARY_POISONED)?;
            let profile = guard
                .as_ref()
                .ok_or(LIBRARY_LOADING)?
                .profile(&manufacturer, &model)?;
            Ok(json!({"v": 0, "type": "library.profile", "profile": profile}))
        }
    }
}

/// LIB-14: the reply to one library request, an `error` message when it fails or the library
/// is still loading. Never panics, also on a poisoned lock.
pub fn answer_library(lib: &RwLock<Option<Library>>, req: LibraryRequest) -> String {
    match library_reply(lib, req) {
        Ok(reply) => reply.to_string(),
        Err(e) => error(&e),
    }
}

/// A server for tests that never use the library: it stays "loading".
#[cfg(test)]
pub fn spawn_server(port: u16) -> std::io::Result<(Receiver<Event>, SocketAddr)> {
    spawn_server_with(port, Arc::new(RwLock::new(None)))
}

/// IPC-01: listen on loopback (port 0 = ephemeral); events flow to the tick, library
/// requests are answered by the client threads from `library` (LIB-14).
pub fn spawn_server_with(
    port: u16,
    library: SharedLibrary,
) -> std::io::Result<(Receiver<Event>, SocketAddr)> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
    let addr = listener.local_addr()?;
    let (events, rx) = channel();
    std::thread::Builder::new()
        .name("ipc-listener".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let events = events.clone();
                let library = Arc::clone(&library);
                // A failed spawn only loses this one client.
                let _ = std::thread::Builder::new()
                    .name("ipc-client".into())
                    .spawn(move || {
                        // IPC-07: a panic closes only this client.
                        if catch_unwind(AssertUnwindSafe(|| {
                            serve_client(stream, &events, &library)
                        }))
                        .is_err()
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

/// PAT-09: a `patch.add` of a library type (`"<manufacturer>/<model>"`) takes the library's
/// profile along; any other command, or a key the library does not have, goes on unchanged.
pub fn attach_profile(lib: &RwLock<Option<Library>>, cmd: Command) -> Command {
    let Command::PatchAdd { add, profile: None } = cmd else {
        return cmd;
    };
    let profile = add.profile.split_once('/').zip(lib.read().ok()).and_then(
        |((manufacturer, model), guard)| guard.as_ref()?.profile(manufacturer, model).ok(),
    );
    Command::PatchAdd { add, profile }
}

fn serve_client(stream: TcpStream, events: &Sender<Event>, library: &RwLock<Option<Library>>) {
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
                Ok(Command::Library(req)) => {
                    if ws
                        .send(Message::text(answer_library(library, req)))
                        .is_err()
                    {
                        return;
                    }
                }
                Ok(cmd) => {
                    if events
                        .send(Event::Command(attach_profile(library, cmd)))
                        .is_err()
                    {
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
        let v = parse(&snapshot(&engine(), &[]));
        assert_eq!(v["v"], 0);
        assert_eq!(v["type"], "snapshot");
        assert_eq!(v["faders_per_page"], 8);
        assert_eq!(v["grand_master"], 1.0);
        assert_eq!(v["playbacks"], json!([{"id":1,"level":0.0,"active":false}]));
    }

    /// IPC-20: three nodes in three states.
    fn outputs() -> Vec<OutputStatus> {
        use crate::output::OutputState;
        vec![
            OutputStatus {
                name: "art-net",
                state: OutputState::Ok,
            },
            OutputStatus {
                name: "sacn",
                state: OutputState::Degraded,
            },
            OutputStatus {
                name: "serial",
                state: OutputState::NoInterface,
            },
        ]
    }

    #[test]
    fn ipc_20_snapshot_lists_the_output_nodes() {
        let v = parse(&snapshot(&engine(), &outputs()));
        assert_eq!(
            v["outputs"],
            json!([{"name":"art-net","state":"Ok"},
                   {"name":"sacn","state":"Degraded"},
                   {"name":"serial","state":"Degraded","detail":"no interface"}])
        );
        assert_eq!(parse(&snapshot(&engine(), &[]))["outputs"], json!([]));
    }

    #[test]
    fn ipc_20_outputs_delta_only_when_a_state_changes() {
        use crate::output::OutputState;
        let e = engine();
        let mut outs = outputs();
        let mut published = Published::new(&e, &outs);
        assert!(published.diff(&e, &outs).is_empty());
        outs[0].state = OutputState::Stopped;
        let d = published.diff(&e, &outs);
        assert_eq!(d.len(), 1);
        let v = parse(&d[0]);
        assert_eq!(v["v"], 0);
        assert_eq!(v["type"], "outputs");
        assert_eq!(v["outputs"][0], json!({"name":"art-net","state":"Stopped"}));
        assert_eq!(v["outputs"].as_array().unwrap().len(), 3);
        assert!(published.diff(&e, &outs).is_empty(), "no change, no delta");
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
        let v = parse(&snapshot(&e, &[]));
        assert_eq!(v["fixtures"], json!([dimmer_entry()]));
        assert_eq!(v["groups"], json!([{"id":1,"fixtures":[1]}]));
        assert_eq!(
            v["programmer"],
            json!({"selection":[],"targets":[],"active_group":"I",
                   "attribute_groups": no_values(false),"wheels":[],"in_programmer":[],"colour_model":"Hsi"})
        );
    }

    /// PAT-09: the test engine's one dimmer as a snapshot entry.
    fn dimmer_entry() -> Value {
        json!({"id":1,"number":1,"name":"","profile":"dimmer","mode":"default","universe":1,
               "address":1,"footprint":1,"virtual_dimmer":false,"handle":{"page":1,"slot":1}})
    }

    /// Showtec LED Par 56 "6 Channel" with a virtual dimmer (PAT-01's example).
    const SHOWTEC: &str = r#"{"manufacturer":"Showtec","model":"LED Par 56","source":"channel-list","modes":[{"name":"6 Channel","footprint":6,"virtual_dimmer":true,"colour":{"system":"RGB","components":[{"channel":1},{"channel":2},{"channel":3}]},"attributes":[{"name":"Mode","group":"S","kind":"Ltp","channel":4},{"name":"Speed","group":"S","kind":"Ltp","channel":5},{"name":"Strobe","group":"I","kind":"Ltp","channel":6}]}]}"#;

    #[test]
    fn pat_09_snapshot_fixture_entries() {
        let mut e = engine();
        e.add_profile("Showtec/LED Par 56", Profile::from_json(SHOWTEC).unwrap())
            .unwrap();
        e.patch_add(&cmdmx_core::patch::PatchAdd {
            profile: "Showtec/LED Par 56".into(),
            mode: None,
            count: 1,
            universe: 1,
            address: Some(37),
            number: Some(221),
        })
        .unwrap();
        e.patch_name(2, "Front left").unwrap();
        let v = parse(&snapshot(&e, &[]));
        assert_eq!(
            v["fixtures"],
            json!([dimmer_entry(),
                   {"id":2,"number":221,"name":"Front left","profile":"Showtec/LED Par 56",
                    "mode":"6 Channel","universe":1,"address":37,"footprint":6,
                    "virtual_dimmer":true,"handle":{"page":1,"slot":2}}])
        );
    }

    #[test]
    fn pat_09_fixtures_delta_after_a_patch_change_only() {
        let mut e = engine();
        let mut published = Published::new(&e, &[]);
        e.tick(0.0);
        assert!(published.diff(&e, &[]).is_empty());
        e.patch_renumber(1, 42).unwrap();
        e.tick(0.025);
        let d: Vec<Value> = published
            .diff(&e, &[])
            .iter()
            .map(|m| parse(m))
            .filter(|m| m["type"] == "fixtures")
            .collect();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0]["v"], 0);
        assert_eq!(d[0]["fixtures"][0]["number"], 42);
        e.tick(0.05);
        assert!(published.diff(&e, &[]).is_empty(), "no change, no delta");
        e.patch_move(1, 2, 100).unwrap();
        let d = published.diff(&e, &[]);
        assert_eq!(d.len(), 1);
        assert_eq!(parse(&d[0])["fixtures"][0]["universe"], 2);
        assert_eq!(parse(&d[0])["fixtures"][0]["address"], 100);
    }

    #[test]
    fn pat_09_patch_commands_parse() {
        use cmdmx_core::patch::{Handle, PatchAdd};
        let ok = |s: &str| parse_command(s).unwrap();
        assert_eq!(
            ok(
                r#"{"v":0,"cmd":"patch.add","profile":"Showtec/LED Par 56","mode":"6 Channel","count":6,"universe":1,"address":37,"number":221}"#
            ),
            Command::PatchAdd {
                add: PatchAdd {
                    profile: "Showtec/LED Par 56".into(),
                    mode: Some("6 Channel".into()),
                    count: 6,
                    universe: 1,
                    address: Some(37),
                    number: Some(221),
                },
                profile: None,
            }
        );
        // PAT-03: address and number are optional.
        assert_eq!(
            ok(
                r#"{"v":0,"cmd":"patch.add","profile":"rgbw","mode":"default","count":2,"universe":1}"#
            ),
            Command::PatchAdd {
                add: PatchAdd {
                    profile: "rgbw".into(),
                    mode: Some("default".into()),
                    count: 2,
                    universe: 1,
                    address: None,
                    number: None,
                },
                profile: None,
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"patch.move","id":3,"universe":2,"address":101}"#),
            Command::PatchMove {
                id: 3,
                universe: 2,
                address: 101
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"patch.remove","ids":[3,4]}"#),
            Command::PatchRemove { ids: vec![3, 4] }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"patch.renumber","id":3,"number":1201}"#),
            Command::PatchRenumber {
                id: 3,
                number: 1201
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"patch.name","id":3,"name":"Front left"}"#),
            Command::PatchName {
                id: 3,
                name: "Front left".into()
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"patch.change_mode","id":3,"mode":"Big"}"#),
            Command::PatchChangeMode {
                id: 3,
                mode: "Big".into()
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"patch.handle","id":3,"page":2,"slot":5}"#),
            Command::PatchHandle {
                id: 3,
                handle: Handle { page: 2, slot: 5 }
            }
        );
    }

    #[test]
    fn pat_09_bad_patch_commands_are_errors() {
        for bad in [
            r#"{"v":0,"cmd":"patch.add","mode":"6 Channel","count":6,"universe":1}"#,
            r#"{"v":0,"cmd":"patch.add","profile":"rgbw","count":6,"universe":1}"#,
            r#"{"v":0,"cmd":"patch.add","profile":"rgbw","mode":"default","universe":1}"#,
            r#"{"v":0,"cmd":"patch.add","profile":"rgbw","mode":"default","count":6}"#,
            r#"{"v":0,"cmd":"patch.add","profile":"rgbw","mode":"default","count":6,"universe":1,"address":"37"}"#,
            r#"{"v":0,"cmd":"patch.add","profile":"rgbw","mode":"default","count":6,"universe":1,"address":70000}"#,
            r#"{"v":0,"cmd":"patch.move","id":3,"universe":2}"#,
            r#"{"v":0,"cmd":"patch.remove","ids":3}"#,
            r#"{"v":0,"cmd":"patch.remove","ids":["3"]}"#,
            r#"{"v":0,"cmd":"patch.renumber","id":3}"#,
            r#"{"v":0,"cmd":"patch.name","id":3,"name":7}"#,
            r#"{"v":0,"cmd":"patch.change_mode","id":3}"#,
            r#"{"v":0,"cmd":"patch.handle","id":3,"page":2}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn pat_09_patch_add_takes_its_profile_from_the_library() {
        let lib = library("patch");
        let cmd = parse_command(
            r#"{"v":0,"cmd":"patch.add","profile":"Showtec/LED Par 56","mode":"6 Channel","count":6,"universe":1,"address":37,"number":221}"#,
        )
        .unwrap();
        match attach_profile(&lib, cmd) {
            Command::PatchAdd {
                profile: Some(p), ..
            } => {
                assert_eq!(
                    (p.manufacturer.as_str(), p.model.as_str()),
                    ("Showtec", "LED Par 56")
                );
                assert_eq!(p.modes[0].name, "6 Channel");
            }
            other => panic!("expected a patch.add with a profile, got {other:?}"),
        }
        // A key that is not in the library (a show profile such as the demo's) goes as is.
        let cmd = parse_command(
            r#"{"v":0,"cmd":"patch.add","profile":"rgbw","mode":"default","count":1,"universe":1}"#,
        )
        .unwrap();
        assert_eq!(attach_profile(&lib, cmd.clone()), cmd);
        // While the library loads, nothing is attached either.
        let loading = std::sync::RwLock::new(None);
        let cmd = parse_command(
            r#"{"v":0,"cmd":"patch.add","profile":"Showtec/LED Par 56","mode":"6 Channel","count":1,"universe":1}"#,
        )
        .unwrap();
        assert_eq!(attach_profile(&loading, cmd.clone()), cmd);
        // Other commands pass through untouched.
        assert_eq!(attach_profile(&lib, Command::All), Command::All);
    }

    #[test]
    fn ipc_11_programmer_state_object() {
        let mut e = engine();
        e.select_fixture(1).unwrap();
        let v = parse(&snapshot(&e, &[]));
        assert_eq!(
            v["programmer"],
            json!({"selection":[1],"targets":[1],"active_group":"I",
                   "attribute_groups": no_values(true),
                   "wheels":[{"attribute":"Dimmer","value":0.0,"touched":false,"sixteen_bit":false}],
                   "in_programmer":[],"colour_model":"Hsi"})
        );
        e.turn_wheel("Dimmer", 255, Resolution::Normal).unwrap();
        let v = parse(&snapshot(&e, &[]));
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
        let mut published = Published::new(&e, &[]);
        e.tick(0.0);
        assert!(published.diff(&e, &[]).is_empty());

        e.select_fixture(1).unwrap();
        e.tick(0.025);
        let d = published.diff(&e, &[]);
        assert_eq!(d.len(), 1);
        let p = parse(&d[0]);
        assert_eq!(p["v"], 0);
        assert_eq!(p["type"], "programmer");
        assert_eq!(p["selection"], json!([1]));
        assert_eq!(p["wheels"][0]["attribute"], "Dimmer");

        e.tick(0.05);
        assert!(published.diff(&e, &[]).is_empty(), "no change, no delta");

        e.record_group(4).unwrap();
        e.tick(0.075);
        let d = published.diff(&e, &[]);
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
        let mut published = Published::new(&e, &[]);
        e.tick(0.0);
        assert!(published.diff(&e, &[]).is_empty());
        e.set_colour_model(ColourModel::Cmy);
        e.tick(0.025);
        let d = published.diff(&e, &[]);
        assert_eq!(d.len(), 1);
        let p = parse(&d[0]);
        assert_eq!(p["type"], "programmer");
        assert_eq!(p["colour_model"], "Cmy");
    }

    /// A library over the LIB-09 church files with its own empty user folder.
    fn library(test: &str) -> std::sync::RwLock<Option<crate::library::Library>> {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/qlcplus");
        let user = std::env::temp_dir().join(format!("cmdmx-ipc-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&user);
        std::sync::RwLock::new(Some(crate::library::Library::load(Some(&dir), &user).0))
    }

    fn ask(lib: &std::sync::RwLock<Option<crate::library::Library>>, text: &str) -> Value {
        match parse_command(text).unwrap() {
            Command::Library(req) => parse(&answer_library(lib, req)),
            other => panic!("expected a library request, got {other:?}"),
        }
    }

    #[test]
    fn lib_14_search_answers_the_asking_client() {
        let lib = library("search");
        let v = ask(&lib, r#"{"v":0,"cmd":"library.search","query":"shehds"}"#);
        assert_eq!(
            v,
            json!({"v":0,"type":"library.results","items":[{"manufacturer":"Shehds",
                   "model":"LED Beam+Wash 19x15W RGBW Zoom Lighting",
                   "modes":["16 Channel","24 Channel"],"source":"qlcplus"}]})
        );
        let loading = std::sync::RwLock::new(None);
        let v = ask(&loading, r#"{"v":0,"cmd":"library.search","query":"x"}"#);
        assert_eq!(v["type"], "error");
        assert!(v["message"].as_str().unwrap().contains("loading"), "{v}");
    }

    #[test]
    fn lib_14_report_profile_and_saves() {
        let lib = library("saves");
        let v = ask(
            &lib,
            r#"{"v":0,"cmd":"library.import_report","manufacturer":"Showtec","model":"LED Par 56"}"#,
        );
        assert_eq!(v["type"], "library.import_report");
        assert_eq!(v["report"]["channels"].as_array().unwrap().len(), 6);
        assert_eq!(v["report"]["channels"][5]["source_name"], "Mode");
        let v = ask(
            &lib,
            r#"{"v":0,"cmd":"library.profile","manufacturer":"Showtec","model":"LED Par 56"}"#,
        );
        assert_eq!(v["type"], "library.profile");
        assert_eq!(v["profile"]["modes"][0]["name"], "6 Channel");
        let mut profile = v["profile"].clone();
        profile["model"] = json!("LED Par 56 (church)");
        let v = ask(
            &lib,
            &json!({"v":0,"cmd":"library.save_profile","profile":profile}).to_string(),
        );
        assert_eq!(
            v,
            json!({"v":0,"type":"library.saved","manufacturer":"Showtec","model":"LED Par 56 (church)"})
        );
        let v = ask(
            &lib,
            r#"{"v":0,"cmd":"library.save_channel_list","manufacturer":"U'King","model":"RGBL Par","mode":"4 Channel","channels":["Red","Green","Blue","Lime"]}"#,
        );
        assert_eq!(
            v,
            json!({"v":0,"type":"library.saved","manufacturer":"U'King","model":"RGBL Par"})
        );
        let v = ask(
            &lib,
            r#"{"v":0,"cmd":"library.profile","manufacturer":"Nope","model":"Nope"}"#,
        );
        assert_eq!(v["type"], "error");
    }

    #[test]
    fn lib_14_bad_library_requests_are_errors() {
        for bad in [
            r#"{"v":0,"cmd":"library.search"}"#,
            r#"{"v":0,"cmd":"library.import_report","manufacturer":"Showtec"}"#,
            r#"{"v":0,"cmd":"library.profile","model":"LED Par 56"}"#,
            r#"{"v":0,"cmd":"library.save_profile","profile":{"modes":"x"}}"#,
            r#"{"v":0,"cmd":"library.save_channel_list","manufacturer":"A","model":"B","mode":"C","channels":["Plasma"]}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
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
        let mut published = Published::new(&e, &[]);
        e.tick(0.0);
        assert!(published.diff(&e, &[]).is_empty());

        e.set_fader(1, 0.5, 0.025).unwrap();
        e.tick(0.025);
        let d = published.diff(&e, &[]);
        assert_eq!(d.len(), 1);
        assert_eq!(
            parse(&d[0]),
            json!({"v":0,"type":"playback","id":1,"level":0.5,"active":true})
        );
        e.tick(0.05);
        assert!(published.diff(&e, &[]).is_empty(), "no change, no delta");

        e.set_grand_master(0.25).unwrap();
        let d = published.diff(&e, &[]);
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
        tx.send(snapshot(&engine(), &[])).unwrap();
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

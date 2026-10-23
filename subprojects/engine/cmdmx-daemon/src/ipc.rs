//! IPC v0: WebSocket server, JSON messages and per-tick deltas (IPC-01..07), plus the
//! programmer snapshot, deltas and commands (IPC-10..13, VS2-07), and the fixture library
//! requests (LIB-14, VS2-11), the patch commands and `fixtures` delta (PAT-09, VS2-12), and
//! the output node states in the snapshot and the `outputs` delta (IPC-20), and the programmer
//! contents, picker colour and `programmer.colour` command (IPC-21..23), and the show state and
//! `show.*` commands (SHW-15), and the output nodes: their fields and states in the same
//! snapshot and delta, the `output.*` commands (OUT-33, OUT-34) and `output.ports` (OUT-35).
//!
//! One thread per client. The tick only talks to clients through channels and
//! never blocks: commands arrive on an unbounded channel, outgoing messages
//! leave through a bounded queue per client (IPC-07). Library requests never reach the tick:
//! the client thread answers them itself, straight to the asking client. So does `show.list`,
//! and `output.ports` (the serial ports are listed there, never on the tick),
//! and `show.open` is read from disk there too, so the tick never waits on a show file.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender, TryRecvError};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use cmdmx_core::attribute::AttributeGroup;
use cmdmx_core::colour::ColourModel;
use cmdmx_core::colour::TargetColour;
use cmdmx_core::engine::{Engine, PlaybackSettingsChange};
use cmdmx_core::library::channel_list::ChannelList;
use cmdmx_core::output::{Node, NodeId, NodeKind};
use cmdmx_core::palette::{
    AutoIcon, Cell, PaletteId, PaletteKind, Record, RecordBy, RecordMode, Timing,
};
use cmdmx_core::patch::{Fixture, FixtureId, Handle, PatchAdd};
use cmdmx_core::playback::{
    AddCue, BpmFader, Button, ChaseOptions, CueChange, CueNumber, Direction, FaderMode,
    GroupMaster, GroupMode, GroupTime, HandleButton, HandleButtons, KillPoint, Kind, Link,
    ListAction, ListFader, ListOptions, LoopAction, Master, MasterKind, Paging, PlaybackChange,
    PlaybackId, PlaybackRecord, Priority, SpeedSource, Track, UpdateMode, PAGES,
};
use cmdmx_core::profile::Profile;
use cmdmx_core::programmer::{
    ClearLtp, ClearPrecedence, GroupId, ProgrammerSettingsChange, ProgrammerView, Resolution,
};
use cmdmx_core::value::{Stored, Value};
use cmdmx_core::CoreError;
use cmdmx_core::UniverseId;
use serde::{Deserialize, Serialize};
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::http::StatusCode;
use tungstenite::{Error as WsError, Message};

use crate::library::Library;
use crate::output::{OutputState, OutputStatus};
use crate::shows::{valid_name, Opened, ShowRequest, ShowState, Shows};

/// Messages a client may have queued before it is disconnected (IPC-07).
const QUEUE: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// CTL-40: handled by the control layer on the tick, never by the engine.
    Control(ControlRequest),
    /// OUT-34: a new output node; the fields not given are the line's defaults (OUT-25).
    OutputAdd {
        line: UniverseId,
        enabled: bool,
        kind: NodeKind,
    },
    /// OUT-34: only the fields to change (`id`, `v` and `cmd` are not among them).
    OutputEdit {
        id: NodeId,
        change: serde_json::Map<String, serde_json::Value>,
    },
    OutputRemove {
        ids: Vec<NodeId>,
    },
    /// OUT-35: answered in the client thread.
    OutputPorts,
    FaderSet {
        playback: PlaybackId,
        level: f32,
    },
    GrandMasterSet {
        level: f32,
    },
    /// PBK-03: show another page of playbacks.
    PageSet {
        page: u16,
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
    /// IPC-32: a masked and/or timed Clear.
    ClearWith {
        groups: Option<Vec<AttributeGroup>>,
        time: Option<f32>,
    },
    Locate {
        groups: Option<Vec<AttributeGroup>>,
    },
    Highlight {
        on: bool,
    },
    /// `groups` and `attribute` are never both set; neither means the active group.
    Off {
        groups: Option<Vec<AttributeGroup>>,
        attribute: Option<String>,
    },
    RemDim,
    SettingsProgrammer(ProgrammerSettingsChange),
    AttributeGroup {
        group: AttributeGroup,
    },
    Wheel {
        attribute: String,
        steps: i32,
        resolution: Resolution,
    },
    /// IPC-23
    Colour {
        hue: f32,
        saturation: f32,
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
    /// SHW-15: `New` and `SaveAs` reach the tick once their name is checked; `List` and `Open`
    /// are answered by the client thread.
    Show(ShowRequest),
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
    /// PAL-27: the palette commands name palettes by number and cells by window and handle.
    PaletteRecord(Record),
    PaletteApply {
        number: u32,
        timing: Timing,
    },
    PaletteInclude {
        number: u32,
    },
    PaletteCopy {
        number: u32,
        to: Cell,
    },
    PaletteMove {
        number: u32,
        to: Cell,
    },
    PaletteRenumber {
        number: u32,
        to: u32,
    },
    PaletteDelete {
        number: u32,
    },
    PaletteLegend {
        number: u32,
        legend: String,
        icon: Option<String>,
    },
    SettingsPalette {
        time: Option<f32>,
        overlap: Option<f32>,
        record_by: Option<RecordBy>,
    },
    /// PBK-36: the playback commands name playbacks by id and handles by page and slot.
    PlaybackRecord(PlaybackRecord),
    PlaybackFlash {
        id: PlaybackId,
        on: bool,
    },
    PlaybackSwop {
        id: PlaybackId,
        on: bool,
    },
    PlaybackLatch {
        id: PlaybackId,
    },
    PlaybackRelease {
        id: PlaybackId,
    },
    /// `playback.options` and `playback.times` both land here.
    PlaybackChange {
        id: PlaybackId,
        change: PlaybackChange,
    },
    PlaybackLegend {
        id: PlaybackId,
        legend: String,
        icon: Option<String>,
        halo: Option<TargetColour>,
    },
    PlaybackCopy {
        id: PlaybackId,
        page: u16,
        slot: u16,
    },
    PlaybackMove {
        id: PlaybackId,
        page: u16,
        slot: u16,
    },
    PlaybackDelete {
        id: PlaybackId,
    },
    PageLegend {
        page: u16,
        legend: String,
    },
    DboSet {
        on: bool,
    },
    SettingsPlayback(PlaybackSettingsChange),
    /// CUE-41: `id` `None` is the connected list (CUE-27).
    CuelistAction {
        id: Option<PlaybackId>,
        action: ListAction,
    },
    CuelistConnect {
        id: PlaybackId,
    },
    /// IPC-25: `at` is the receive time, set by `stamp`.
    PlaybackButton {
        id: PlaybackId,
        button: HandleButton,
        on: bool,
        at: f64,
    },
    PlaybackTap {
        id: PlaybackId,
        at: f64,
    },
    PlaybackBpm {
        id: PlaybackId,
        bpm: f32,
    },
    /// `playback.options` with a `list`, `chase` or `master` object: merged with the stored
    /// value by the tick (IPC-27).
    PlaybackOptions {
        id: PlaybackId,
        change: PlaybackChange,
        list: Option<ListPatch>,
        chase: Option<ChasePatch>,
        master: Option<MasterPatch>,
    },
    /// `cue.times`, `cue.legend` and `cue.track` all land here.
    CueEdit {
        id: PlaybackId,
        cue: CueNumber,
        change: CueChange,
    },
    CueDelete {
        id: PlaybackId,
        cue: CueNumber,
    },
    CueRenumber {
        id: PlaybackId,
        cue: CueNumber,
        to: CueNumber,
    },
    CueRenumberAll {
        id: PlaybackId,
    },
    CueCopy {
        id: PlaybackId,
        cue: CueNumber,
        to: CueNumber,
    },
    /// CTL-40: handed to the tick as `Event::ControlMapping`, never applied.
    ControlMapping {
        name: String,
    },
    /// IPC-24: handed to the tick as `Event::CueView`, never applied.
    CueView {
        id: PlaybackId,
        cue: CueNumber,
    },
    CueRemove {
        id: PlaybackId,
        cue: CueNumber,
        fixture: FixtureId,
        attribute: Option<String>,
    },
    /// IPC-26: `number` `None` is the lowest free one.
    MasterRecord {
        page: u16,
        slot: u16,
        kind: MasterKind,
        number: Option<u8>,
    },
    /// GRM-15: a group master for a group, on an empty handle.
    GroupMasterRecord {
        page: u16,
        slot: u16,
        group: GroupId,
    },
    /// IPC-30: handled by the tick's `run`, never by `apply`.
    Undo,
    Redo,
}

/// IPC-31: the labels of the next undo and redo step, `None` when there is none.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct UndoInfo {
    pub undo: Option<String>,
    pub redo: Option<String>,
}

/// IPC-27: the fields of `playback.options` `list`, each optional.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ListPatch {
    tracking: Option<bool>,
    fader: Option<ListFader>,
    fire_first_cue: Option<bool>,
    #[serde(rename = "loop")]
    looping: Option<LoopAction>,
}

impl ListPatch {
    pub fn over(&self, mut o: ListOptions) -> ListOptions {
        o.tracking = self.tracking.unwrap_or(o.tracking);
        o.fader = self.fader.unwrap_or(o.fader);
        o.fire_first_cue = self.fire_first_cue.unwrap_or(o.fire_first_cue);
        o.looping = self.looping.unwrap_or(o.looping);
        o
    }
}

/// IPC-27: the fields of `playback.options` `chase`, each optional.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChasePatch {
    bpm: Option<f32>,
    crossfade: Option<f32>,
    direction: Option<Direction>,
    #[serde(rename = "loop")]
    looping: Option<LoopAction>,
    speed_source: Option<SpeedSource>,
}

impl ChasePatch {
    pub fn over(&self, mut o: ChaseOptions) -> ChaseOptions {
        o.bpm = self.bpm.unwrap_or(o.bpm);
        o.crossfade = self.crossfade.unwrap_or(o.crossfade);
        o.direction = self.direction.unwrap_or(o.direction);
        o.looping = self.looping.unwrap_or(o.looping);
        o.speed_source = self.speed_source.unwrap_or(o.speed_source);
        o
    }
}

/// IPC-27: the fields of `playback.options` `master`, each optional; the ones that do not
/// belong to the stored master's kind are refused.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MasterPatch {
    number: Option<u8>,
    bpm: Option<f32>,
    multiplier: Option<f32>,
    fader: Option<BpmFader>,
    fader_max: Option<f32>,
    rate_scale: Option<u16>,
    /// GRM-15: `Unknown` is refused when the command is parsed.
    mode: Option<GroupMode>,
    scale: Option<u16>,
    enabled: Option<bool>,
}

impl MasterPatch {
    pub fn over(&self, master: Master) -> Result<Master, CoreError> {
        let wrong = |what: &str| CoreError::InvalidOption(format!("{what} (not on a {master})"));
        let group_only = self.mode.is_some() || self.scale.is_some() || self.enabled.is_some();
        match master.clone() {
            Master::Bpm(mut m) => {
                if self.rate_scale.is_some() || group_only {
                    return Err(wrong("rate_scale, mode, scale or enabled"));
                }
                m.number = self.number.unwrap_or(m.number);
                m.bpm = self.bpm.unwrap_or(m.bpm);
                m.multiplier = self.multiplier.unwrap_or(m.multiplier);
                m.fader = self.fader.unwrap_or(m.fader);
                m.fader_max = self.fader_max.unwrap_or(m.fader_max);
                Ok(Master::Bpm(m))
            }
            Master::Rate(mut m) => {
                let bpm_only = [self.bpm, self.multiplier, self.fader_max];
                if self.fader.is_some() || bpm_only.iter().any(Option::is_some) || group_only {
                    return Err(wrong("a BPM, mode, scale or enabled field"));
                }
                m.number = self.number.unwrap_or(m.number);
                m.rate_scale = self.rate_scale.unwrap_or(m.rate_scale);
                Ok(Master::Rate(m))
            }
            Master::Group(mut m) => {
                let other = [self.bpm, self.multiplier, self.fader_max];
                if self.number.is_some()
                    || self.fader.is_some()
                    || self.rate_scale.is_some()
                    || other.iter().any(Option::is_some)
                {
                    return Err(wrong("a speed master field"));
                }
                m.mode = self.mode.unwrap_or(m.mode);
                m.scale = self.scale.unwrap_or(m.scale);
                m.enabled = self.enabled.unwrap_or(m.enabled);
                Ok(Master::Group(m))
            }
        }
    }
}

/// CTL-40: the `control.*` commands. `control.mapping` is answered through `Event::ControlMapping`.
#[derive(Debug, Clone, PartialEq)]
pub enum ControlRequest {
    Enable {
        device: String,
        on: bool,
    },
    Assign {
        device: String,
        mapping: String,
    },
    Learn {
        action: cmdmx_control::action::Action,
        mapping: Option<String>,
    },
    LearnCancel,
    /// CTL-31: the order of the Mackie units.
    Order {
        devices: Vec<String>,
    },
    /// `change` holds `input` and the fields to change; they go over the stored binding.
    Binding {
        mapping: String,
        change: serde_json::Map<String, serde_json::Value>,
    },
    Unbind {
        mapping: String,
        input: cmdmx_control::mapping::Input,
    },
    MappingNew {
        name: String,
        device: String,
    },
    MappingRename {
        name: String,
        to: String,
    },
    MappingDelete {
        name: String,
    },
    MappingImport {
        json: String,
        replace: bool,
    },
    /// The settings to change (CTL-35); checked as a whole when applied, a refusal changes nothing.
    Osc(serde_json::Map<String, serde_json::Value>),
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
    /// SHW-15: a show read by the client thread, to be switched to.
    ShowOpened(Box<Opened>),
    /// SHW-15: a refused show request, for every client (IPC-06).
    Warning(String),
    /// IPC-24: the answer goes to `reply`, the asking client's own queue, and nowhere else.
    CueView {
        id: PlaybackId,
        cue: CueNumber,
        reply: SyncSender<String>,
    },
    /// CTL-40: the answer to `control.mapping`, to the asking client only.
    ControlMapping {
        name: String,
        reply: SyncSender<String>,
    },
}

/// An unsigned integer field that fits `T`.
fn uint<T: TryFrom<u64>>(v: &serde_json::Value, key: &str) -> Result<T, String> {
    v.get(key)
        .and_then(|n| n.as_u64())
        .and_then(|n| T::try_from(n).ok())
        .ok_or_else(|| format!("missing or invalid \"{key}\""))
}

/// A number field in 0.0..=1.0.
fn unit(v: &serde_json::Value, key: &str) -> Result<f32, String> {
    v.get(key)
        .and_then(|n| n.as_f64())
        .map(|n| n as f32)
        .filter(|n| (0.0..=1.0).contains(n))
        .ok_or_else(|| format!("missing or invalid \"{key}\" (expected 0.0 to 1.0)"))
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

/// An optional number field: absent is `None`, present but not a number is an error.
fn opt_f32(v: &serde_json::Value, key: &str) -> Result<Option<f32>, String> {
    match v.get(key) {
        None => Ok(None),
        Some(n) => n
            .as_f64()
            .map(|n| Some(n as f32))
            .ok_or_else(|| format!("invalid \"{key}\" (expected a number)")),
    }
}

/// A field that deserialises as `T` (a cell, an enum).
fn field<T: serde::de::DeserializeOwned>(
    v: &serde_json::Value,
    key: &str,
    expected: &str,
) -> Result<T, String> {
    v.get(key)
        .and_then(|f| serde_json::from_value(f.clone()).ok())
        .ok_or_else(|| format!("missing or invalid \"{key}\" (expected {expected})"))
}

/// A number field (any f32).
fn number(v: &serde_json::Value, key: &str) -> Result<f32, String> {
    opt_f32(v, key)?.ok_or_else(|| format!("missing \"{key}\""))
}

/// CUE-02: a cue number, a JSON number of 0.001–9999.999 with at most three decimals.
fn cue_number(v: &serde_json::Value, key: &str) -> Result<CueNumber, String> {
    (v.get(key).and_then(|n| n.as_f64()).and_then(CueNumber::new)).ok_or_else(|| {
        format!("missing or invalid \"{key}\" (expected a cue number, 0.001 to 9999.999)")
    })
}

fn opt_cue_number(v: &serde_json::Value, key: &str) -> Result<Option<CueNumber>, String> {
    match v.get(key) {
        None => Ok(None),
        Some(_) => cue_number(v, key).map(Some),
    }
}

/// A boolean field.
fn boolean(v: &serde_json::Value, key: &str) -> Result<bool, String> {
    v.get(key)
        .and_then(|b| b.as_bool())
        .ok_or_else(|| format!("missing or invalid \"{key}\" (expected true or false)"))
}

/// An optional field that deserialises as `T`: absent is `None`, present but invalid is an error.
fn opt_field<T: serde::de::DeserializeOwned>(
    v: &serde_json::Value,
    key: &str,
    expected: &str,
) -> Result<Option<T>, String> {
    match v.get(key) {
        None => Ok(None),
        Some(_) => field(v, key, expected).map(Some),
    }
}

/// A nullable optional field (PBK-36): absent is `None`, `null` is `Some(None)`.
fn nullable<T: serde::de::DeserializeOwned>(
    v: &serde_json::Value,
    key: &str,
    expected: &str,
) -> Result<Option<Option<T>>, String> {
    match v.get(key) {
        None => Ok(None),
        Some(serde_json::Value::Null) => Ok(Some(None)),
        Some(_) => field(v, key, expected).map(|x| Some(Some(x))),
    }
}

/// PBK-36: the handle `{"page":2,"slot":4}` under `to`.
fn target_handle(v: &serde_json::Value) -> Result<(u16, u16), String> {
    let to = v
        .get("to")
        .ok_or("missing \"to\" (expected {page, slot})")?;
    Ok((uint(to, "page")?, uint(to, "slot")?))
}

/// PBK-36: the fields of `playback.options` and `playback.times`, all optional.
fn playback_change(v: &serde_json::Value) -> Result<PlaybackChange, String> {
    Ok(PlaybackChange {
        priority: opt_field::<Priority>(
            v,
            "priority",
            "Low, Normal, High, Programmer or VeryHigh",
        )?,
        fader_mode: match v.get("fader_mode") {
            None => None,
            Some(m) => Some(match m.as_u64() {
                Some(0) => FaderMode::Mode0,
                Some(1) => FaderMode::Mode1,
                Some(2) => FaderMode::Mode2,
                _ => return Err("invalid \"fader_mode\" (expected 0, 1 or 2)".into()),
            }),
        },
        kill_point: opt_field::<KillPoint>(v, "kill_point", "FadeOutComplete or FaderAtZero")?,
        release_mask: nullable(v, "release_mask", "an array of attribute groups or null")?,
        release_time: nullable(v, "release_time", "a number or null")?,
        paging: opt_field::<Paging>(v, "paging", "Unlocked, Locked or TransparentLock")?,
        group: nullable(v, "group", "1 to 99 or null")?,
        kind: opt_field::<Kind>(v, "kind", "Cues or Chase")?,
        delay: opt_f32(v, "delay")?,
        fade_in: opt_f32(v, "fade_in")?,
        fade_out: opt_f32(v, "fade_out")?,
        overlap: opt_f32(v, "overlap")?,
        ..PlaybackChange::default()
    })
}

/// OUT-34: start from the line's defaults for the kind, then take the fields that are given.
/// An output number field (OUT-28): the range text of `output::check_number` when out of range.
fn out_int<T: TryFrom<i64>>(v: &serde_json::Value, key: &str) -> Result<Option<T>, String> {
    let Some(n) = v.get(key) else { return Ok(None) };
    let n = n
        .as_i64()
        .ok_or_else(|| format!("missing or invalid \"{key}\""))?;
    cmdmx_core::output::check_number(key, n).map_err(|e| e.to_string())?;
    T::try_from(n)
        .map(Some)
        .map_err(|_| format!("missing or invalid \"{key}\""))
}

fn parse_output_add(v: &serde_json::Value) -> Result<Command, String> {
    let line: UniverseId = out_int(v, "line")?.ok_or("missing or invalid \"line\"")?;
    let mut kind = match v.get("kind").and_then(|k| k.as_str()) {
        Some("artnet") => NodeKind::default_artnet(line),
        Some("sacn") => NodeKind::default_sacn(line),
        Some("serial") => NodeKind::Serial {
            port: "auto".into(),
        },
        _ => return Err("missing or invalid \"kind\" (artnet, sacn or serial)".into()),
    };
    match &mut kind {
        NodeKind::Artnet {
            port_address,
            target,
        } => {
            *port_address = out_int(v, "port_address")?.unwrap_or(*port_address);
            if v.get("target").is_some() {
                *target = string(v, "target")?;
            }
        }
        NodeKind::Sacn {
            universe,
            priority,
            unicast,
        } => {
            *universe = out_int(v, "universe")?.unwrap_or(*universe);
            *priority = out_int(v, "priority")?.unwrap_or(*priority);
            match v.get("unicast") {
                None => {}
                Some(serde_json::Value::Null) => *unicast = None,
                Some(_) => *unicast = Some(string(v, "unicast")?),
            }
        }
        NodeKind::Serial { port } => {
            if v.get("port").is_some() {
                *port = string(v, "port")?;
            }
        }
    }
    let enabled = match v.get("enabled") {
        None => true,
        Some(b) => b.as_bool().ok_or("missing or invalid \"enabled\"")?,
    };
    Ok(Command::OutputAdd {
        line,
        enabled,
        kind,
    })
}

/// OUT-35: the reply to `output.ports`: the `/dev/cu.*` serial ports by path, with what the
/// USB descriptor says and whether OUT-09 would take it as an ENTTEC Pro.
pub fn ports_reply(ports: &[serialport::SerialPortInfo]) -> String {
    use serialport::SerialPortType::UsbPort;
    let mut cu: Vec<_> = ports
        .iter()
        .filter(|p| p.port_name.starts_with("/dev/cu."))
        .collect();
    cu.sort_by(|a, b| a.port_name.cmp(&b.port_name));
    let ports: Vec<_> = cu
        .into_iter()
        .map(|p| match &p.port_type {
            UsbPort(usb) => serde_json::json!({
                "path": p.port_name,
                "product": usb.product,
                "vid": format!("{:04x}", usb.vid),
                "pid": format!("{:04x}", usb.pid),
                "enttec": usb
                    .product
                    .as_deref()
                    .is_some_and(cmdmx_protocols::enttec::is_pro_product_string),
            }),
            _ => serde_json::json!({
                "path": p.port_name,
                "product": null,
                "vid": null,
                "pid": null,
                "enttec": false,
            }),
        })
        .collect();
    serde_json::json!({"v": 0, "type": "ports", "ports": ports}).to_string()
}

/// CTL-40: `control.*` to a command; an unknown action or input is refused here.
fn parse_control(cmd: &str, v: &serde_json::Value) -> Result<Command, String> {
    use cmdmx_control::{action::Action, mapping::Input};
    let input = |v: &serde_json::Value| field::<Input>(v, "input", "a MIDI input or OSC address");
    let action = |text: &str| text.parse::<Action>().map_err(|e| e.to_string());
    Ok(Command::Control(match cmd {
        "enable" => ControlRequest::Enable {
            device: string(v, "device")?,
            on: boolean(v, "on")?,
        },
        "assign" => ControlRequest::Assign {
            device: string(v, "device")?,
            mapping: string(v, "mapping")?,
        },
        "learn" => ControlRequest::Learn {
            action: action(&string(v, "action")?)?,
            mapping: opt_field(v, "mapping", "a mapping name")?,
        },
        "learn_cancel" => ControlRequest::LearnCancel,
        "order" => ControlRequest::Order {
            devices: field(v, "devices", "a list of device names")?,
        },
        "binding" => {
            input(v)?;
            if let Some(a) = v.get("action") {
                action(a.as_str().ok_or("invalid \"action\" (expected text)")?)?;
            }
            let mut change = v.as_object().cloned().unwrap_or_default();
            for key in ["v", "cmd", "mapping"] {
                change.remove(key);
            }
            ControlRequest::Binding {
                mapping: string(v, "mapping")?,
                change,
            }
        }
        "unbind" => ControlRequest::Unbind {
            mapping: string(v, "mapping")?,
            input: input(v)?,
        },
        "mapping_new" => ControlRequest::MappingNew {
            name: string(v, "name")?,
            device: string(v, "device")?,
        },
        "mapping_rename" => ControlRequest::MappingRename {
            name: string(v, "name")?,
            to: string(v, "to")?,
        },
        "mapping_delete" => ControlRequest::MappingDelete {
            name: string(v, "name")?,
        },
        "mapping_import" => ControlRequest::MappingImport {
            json: string(v, "json")?,
            replace: opt_field(v, "replace", "true or false")?.unwrap_or(false),
        },
        "osc" => {
            let mut change = v.as_object().cloned().unwrap_or_default();
            for key in ["v", "cmd"] {
                change.remove(key);
            }
            ControlRequest::Osc(change)
        }
        other => return Err(format!("unknown cmd 'control.{other}'")),
    }))
}

/// IPC-25: set the time the client thread received a tap or button press; other commands pass.
pub fn stamp(cmd: Command, at: f64) -> Command {
    match cmd {
        Command::PlaybackTap { id, .. } => Command::PlaybackTap { id, at },
        Command::PlaybackButton { id, button, on, .. } => {
            Command::PlaybackButton { id, button, on, at }
        }
        other => other,
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
        Some("page.set") => Ok(Command::PageSet {
            page: uint(&v, "page")?,
        }),
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
        Some("undo") => Ok(Command::Undo),
        Some("redo") => Ok(Command::Redo),
        Some("programmer.all") => Ok(Command::All),
        Some("programmer.clear") => {
            let groups = opt_field(&v, "groups", "an array of attribute groups")?;
            let time = opt_f32(&v, "time")?;
            Ok(match (&groups, time) {
                (None, None) => Command::Clear,
                _ => Command::ClearWith { groups, time },
            })
        }
        Some("programmer.locate") => Ok(Command::Locate {
            groups: opt_field(&v, "groups", "an array of attribute groups")?,
        }),
        Some("programmer.highlight") => Ok(Command::Highlight {
            on: boolean(&v, "on")?,
        }),
        Some("programmer.off") => {
            let groups = opt_field(&v, "groups", "an array of attribute groups")?;
            let attribute = opt_field(&v, "attribute", "an attribute name")?;
            if groups.is_some() && attribute.is_some() {
                return Err("give \"groups\" or \"attribute\", not both".into());
            }
            Ok(Command::Off { groups, attribute })
        }
        Some("programmer.rem_dim") => Ok(Command::RemDim),
        Some("settings.programmer") => Ok(Command::SettingsProgrammer(ProgrammerSettingsChange {
            clear_precedence: opt_field(
                &v,
                "clear_precedence",
                "SelectionThenProgrammer, SelectionWithProgrammer or ProgrammerThenSelection",
            )?,
            clear_ltp: opt_field(&v, "clear_ltp", "Release or Freeze")?,
            lowlight_level: opt_f32(&v, "lowlight_level")?,
        })),
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
        Some("programmer.colour") => Ok(Command::Colour {
            hue: unit(&v, "hue")?,
            saturation: unit(&v, "saturation")?,
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
        Some("output.add") => parse_output_add(&v),
        Some("output.edit") => {
            let mut change = v.as_object().cloned().unwrap_or_default();
            for key in ["v", "cmd", "id"] {
                change.remove(key);
            }
            Ok(Command::OutputEdit {
                id: uint(&v, "id")?,
                change,
            })
        }
        Some("output.remove") => Ok(Command::OutputRemove {
            ids: v
                .get("ids")
                .and_then(|t| t.as_array())
                .and_then(|a| a.iter().map(|n| n.as_u64()?.try_into().ok()).collect())
                .ok_or("missing or invalid \"ids\" (expected an array of output ids)")?,
        }),
        Some("output.ports") => Ok(Command::OutputPorts),
        Some("show.list") => Ok(Command::Show(ShowRequest::List)),
        Some("show.new") => Ok(Command::Show(ShowRequest::New {
            name: string(&v, "name")?,
        })),
        Some("show.open") => Ok(Command::Show(ShowRequest::Open {
            name: string(&v, "name")?,
        })),
        Some("show.save_as") => Ok(Command::Show(ShowRequest::SaveAs {
            name: string(&v, "name")?,
        })),
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
        Some("palette.record") => Ok(Command::PaletteRecord(
            serde_json::from_value(v.clone())
                .map_err(|e| format!("invalid palette.record: {e}"))?,
        )),
        Some("palette.apply") => Ok(Command::PaletteApply {
            number: uint(&v, "number")?,
            timing: Timing {
                time: opt_f32(&v, "time")?,
                overlap: opt_f32(&v, "overlap")?,
            },
        }),
        Some("palette.include") => Ok(Command::PaletteInclude {
            number: uint(&v, "number")?,
        }),
        Some("palette.copy") => Ok(Command::PaletteCopy {
            number: uint(&v, "number")?,
            to: field(&v, "to", "{window, handle}")?,
        }),
        Some("palette.move") => Ok(Command::PaletteMove {
            number: uint(&v, "number")?,
            to: field(&v, "to", "{window, handle}")?,
        }),
        Some("palette.renumber") => Ok(Command::PaletteRenumber {
            number: uint(&v, "number")?,
            to: uint(&v, "to")?,
        }),
        Some("palette.delete") => Ok(Command::PaletteDelete {
            number: uint(&v, "number")?,
        }),
        Some("palette.legend") => Ok(Command::PaletteLegend {
            number: uint(&v, "number")?,
            legend: string(&v, "legend")?,
            icon: match v.get("icon") {
                None | Some(serde_json::Value::Null) => None,
                Some(i) => Some(
                    i.as_str()
                        .map(String::from)
                        .ok_or("invalid \"icon\" (expected a string or null)")?,
                ),
            },
        }),
        Some("settings.palette") => Ok(Command::SettingsPalette {
            time: opt_f32(&v, "time")?,
            overlap: opt_f32(&v, "overlap")?,
            record_by: match v.get("record_by") {
                None => None,
                Some(_) => Some(field(&v, "record_by", "Channel or Mixed")?),
            },
        }),
        Some("playback.record") => {
            let (mode, add) = match v.get("mode").map(|m| m.as_str()) {
                None => (None, None),
                Some(Some("Replace")) => (Some(RecordMode::Replace), None),
                Some(Some("Merge")) => (Some(RecordMode::Merge), None),
                Some(Some("Update")) => (Some(RecordMode::QuickMerge), None),
                Some(Some("Append")) => (None, Some(AddCue::Append)),
                Some(Some("Insert")) => (None, Some(AddCue::Insert)),
                Some(_) => {
                    return Err(
                        "invalid \"mode\" (expected Replace, Merge, Update, Append or \
                                Insert)"
                            .into(),
                    )
                }
            };
            let cue = opt_cue_number(&v, "cue")?;
            if add.is_some() && cue.is_some() {
                return Err("\"cue\" cannot go with the mode Append or Insert".into());
            }
            Ok(Command::PlaybackRecord(PlaybackRecord {
                page: uint(&v, "page")?,
                slot: uint(&v, "slot")?,
                mask: opt_field(&v, "mask", "an array of attribute groups")?,
                record_by: opt_field(&v, "record_by", "Channel or Mixed")?,
                mode,
                cue,
                add,
                update_mode: opt_field(&v, "update_mode", "Forwards, CueOnly, Backwards or Both")?,
            }))
        }
        Some("playback.flash") => Ok(Command::PlaybackFlash {
            id: uint(&v, "id")?,
            on: boolean(&v, "on")?,
        }),
        Some("playback.swop") => Ok(Command::PlaybackSwop {
            id: uint(&v, "id")?,
            on: boolean(&v, "on")?,
        }),
        Some("playback.latch") => Ok(Command::PlaybackLatch {
            id: uint(&v, "id")?,
        }),
        Some("playback.release") => Ok(match opt_uint(&v, "id")? {
            Some(id) => Command::PlaybackRelease { id },
            None => Command::CuelistAction {
                id: None,
                action: ListAction::Release,
            },
        }),
        Some("playback.options" | "playback.times") => {
            let id = uint(&v, "id")?;
            let change = playback_change(&v)?;
            let list = opt_field(&v, "list", "an object of cue list options")?;
            let chase = opt_field(&v, "chase", "an object of chase options")?;
            let master: Option<MasterPatch> =
                opt_field(&v, "master", "an object of master options")?;
            if master
                .as_ref()
                .is_some_and(|m| m.mode == Some(GroupMode::Unknown))
            {
                return Err("\"mode\" must be Scale, Htp, Limit or TakeOver".into());
            }
            Ok(if (list.is_none() && chase.is_none()) && master.is_none() {
                Command::PlaybackChange { id, change }
            } else {
                Command::PlaybackOptions {
                    id,
                    change,
                    list,
                    chase,
                    master,
                }
            })
        }
        Some("playback.legend") => Ok(Command::PlaybackLegend {
            id: uint(&v, "id")?,
            legend: string(&v, "legend")?,
            icon: nullable::<String>(&v, "icon", "a string or null")?.flatten(),
            halo: nullable(&v, "halo", "{r, g, b} or null")?.flatten(),
        }),
        Some("playback.copy") => {
            let (page, slot) = target_handle(&v)?;
            Ok(Command::PlaybackCopy {
                id: uint(&v, "id")?,
                page,
                slot,
            })
        }
        Some("playback.move") => {
            let (page, slot) = target_handle(&v)?;
            Ok(Command::PlaybackMove {
                id: uint(&v, "id")?,
                page,
                slot,
            })
        }
        Some("playback.delete") => Ok(Command::PlaybackDelete {
            id: uint(&v, "id")?,
        }),
        Some("page.legend") => Ok(Command::PageLegend {
            page: uint(&v, "page")?,
            legend: string(&v, "legend")?,
        }),
        Some("dbo.set") => Ok(Command::DboSet {
            on: boolean(&v, "on")?,
        }),
        Some("settings.playback") => Ok(Command::SettingsPlayback(PlaybackSettingsChange {
            record_by: opt_field(&v, "record_by", "Channel or Mixed")?,
            delay: opt_f32(&v, "delay")?,
            fade_in: opt_f32(&v, "fade_in")?,
            fade_out: opt_f32(&v, "fade_out")?,
            overlap: opt_f32(&v, "overlap")?,
            release_mask: opt_field(&v, "release_mask", "an array of attribute groups")?,
            release_time: opt_f32(&v, "release_time")?,
            auto_connect: opt_field(&v, "auto_connect", "true or false")?,
            cue_list_fader: opt_field(&v, "cue_list_fader", "KillWithOff or KillAtZero")?,
            update_mode: opt_field(&v, "update_mode", "Forwards, CueOnly, Backwards or Both")?,
            handle_buttons: opt_field(
                &v,
                "handle_buttons",
                "BestPractice, Titan or {\"Custom\": {kind: [upper, lower]}}",
            )?,
        })),
        Some("cuelist.go") => Ok(Command::CuelistAction {
            id: opt_uint(&v, "id")?,
            action: ListAction::Go,
        }),
        Some("cuelist.back") => Ok(Command::CuelistAction {
            id: opt_uint(&v, "id")?,
            action: ListAction::Back,
        }),
        Some("cuelist.stop") => Ok(Command::CuelistAction {
            id: opt_uint(&v, "id")?,
            action: ListAction::Stop,
        }),
        Some("cuelist.goto") => Ok(Command::CuelistAction {
            id: opt_uint(&v, "id")?,
            action: ListAction::Goto(cue_number(&v, "cue")?),
        }),
        Some("cuelist.next") => Ok(Command::CuelistAction {
            id: opt_uint(&v, "id")?,
            action: ListAction::Next(cue_number(&v, "cue")?),
        }),
        Some("cuelist.connect") => Ok(Command::CuelistConnect {
            id: uint(&v, "id")?,
        }),
        Some("playback.button") => Ok(Command::PlaybackButton {
            id: uint(&v, "id")?,
            button: field(&v, "button", "upper or lower")?,
            on: boolean(&v, "on")?,
            at: 0.0,
        }),
        Some("playback.tap") => Ok(Command::PlaybackTap {
            id: uint(&v, "id")?,
            at: 0.0,
        }),
        Some("playback.bpm") => Ok(Command::PlaybackBpm {
            id: uint(&v, "id")?,
            bpm: number(&v, "bpm")?,
        }),
        Some("cue.times") => Ok(Command::CueEdit {
            id: uint(&v, "id")?,
            cue: cue_number(&v, "cue")?,
            change: CueChange {
                delay: opt_f32(&v, "delay")?,
                fade_in: opt_f32(&v, "fade_in")?,
                fade_out: opt_f32(&v, "fade_out")?,
                overlap: opt_f32(&v, "overlap")?,
                link: opt_field::<Link>(&v, "link", "{type: Wait, After or With, offset}")?,
                group_times: opt_field(&v, "group_times", "{group: {delay, fade}}")?,
                ..CueChange::default()
            },
        }),
        Some("cue.legend") => Ok(Command::CueEdit {
            id: uint(&v, "id")?,
            cue: cue_number(&v, "cue")?,
            change: CueChange {
                legend: Some(string(&v, "legend")?),
                ..CueChange::default()
            },
        }),
        Some("cue.track") => Ok(Command::CueEdit {
            id: uint(&v, "id")?,
            cue: cue_number(&v, "cue")?,
            change: CueChange {
                track: Some(field::<Track>(
                    &v,
                    "track",
                    "Track, Block, CueOnly or Solo",
                )?),
                ..CueChange::default()
            },
        }),
        Some("cue.delete") => Ok(Command::CueDelete {
            id: uint(&v, "id")?,
            cue: cue_number(&v, "cue")?,
        }),
        Some("cue.renumber") => Ok(Command::CueRenumber {
            id: uint(&v, "id")?,
            cue: cue_number(&v, "cue")?,
            to: cue_number(&v, "to")?,
        }),
        Some("cue.renumber_all") => Ok(Command::CueRenumberAll {
            id: uint(&v, "id")?,
        }),
        Some("cue.copy") => Ok(Command::CueCopy {
            id: uint(&v, "id")?,
            cue: cue_number(&v, "cue")?,
            to: cue_number(&v, "to")?,
        }),
        Some("cue.view") => Ok(Command::CueView {
            id: uint(&v, "id")?,
            cue: cue_number(&v, "cue")?,
        }),
        Some("cue.remove") => Ok(Command::CueRemove {
            id: uint(&v, "id")?,
            cue: cue_number(&v, "cue")?,
            fixture: uint(&v, "fixture")?,
            attribute: opt_field(&v, "attribute", "a string")?,
        }),
        Some("master.record") => {
            let (page, slot) = (uint(&v, "page")?, uint(&v, "slot")?);
            let kind = match v.get("kind").and_then(|k| k.as_str()) {
                Some("Bpm") => MasterKind::Bpm,
                Some("Rate") => MasterKind::Rate,
                Some("Group") => {
                    // GRM-15: a group, never a number.
                    if v.get("number").is_some() {
                        return Err("a Group master takes \"group\", not \"number\"".into());
                    }
                    return Ok(Command::GroupMasterRecord {
                        page,
                        slot,
                        group: uint(&v, "group")?,
                    });
                }
                _ => return Err("missing or invalid \"kind\" (expected Bpm, Rate or Group)".into()),
            };
            Ok(Command::MasterRecord {
                page,
                slot,
                kind,
                number: opt_uint(&v, "number")?,
            })
        }
        Some("control.mapping") => Ok(Command::ControlMapping {
            name: string(&v, "name")?,
        }),
        Some(other) => match other.strip_prefix("control.") {
            Some(cmd) => parse_control(cmd, &v),
            None => Err(format!("unknown cmd '{other}'")),
        },
        None => Err("missing \"cmd\"".into()),
    }
}

/// PBK-35: what a playback has stored (the part that changes only by an edit).
#[derive(Serialize, Clone, PartialEq)]
struct StoredInfo {
    id: PlaybackId,
    page: u16,
    slot: u16,
    legend: String,
    icon: Option<String>,
    auto_icon: Option<AutoIcon>,
    /// The effective halo (PBK-33).
    halo: Option<TargetColour>,
    options: OptionsInfo,
    times: TimesInfo,
    /// Attribute groups all cues hold, I P C G B E S FX order.
    groups: Vec<AttributeGroup>,
    /// CUE-40, IPC-28.
    kind: Kind,
    /// Without values.
    cues: Vec<CueInfo>,
    list: ListOptions,
    chase: ChaseOptions,
    master: Option<Master>,
    /// CUE-25: what the two buttons of the handle do now.
    buttons: [Button; 2],
}

/// CUE-40: a cue without its values.
#[derive(Serialize, Clone, PartialEq)]
struct CueInfo {
    number: CueNumber,
    legend: String,
    delay: f32,
    fade_in: f32,
    fade_out: f32,
    overlap: f32,
    link: Link,
    track: Track,
    group_times: BTreeMap<AttributeGroup, GroupTime>,
    /// The attribute groups this cue holds.
    groups: Vec<AttributeGroup>,
}

#[derive(Serialize, Clone, PartialEq)]
struct OptionsInfo {
    priority: Priority,
    fader_mode: FaderMode,
    kill_point: KillPoint,
    release_mask: Option<Vec<AttributeGroup>>,
    release_time: Option<f32>,
    paging: Paging,
    group: Option<u8>,
}

/// The first cue's times.
#[derive(Serialize, Clone, PartialEq)]
struct TimesInfo {
    delay: f32,
    fade_in: f32,
    fade_out: f32,
    overlap: f32,
}

/// PBK-35: the live part, sent as a small `playback` delta when it changes.
#[derive(Serialize, Clone, Copy, PartialEq)]
struct LiveInfo {
    level: f32,
    /// `off`, `active` or `releasing`.
    state: &'static str,
    /// Held or latched.
    flash: bool,
    latched: bool,
    /// Orders the active playbacks by when they fired; `None` while off.
    fired: Option<u64>,
    /// CUE-40: a cue list's or chase's current and next cue; `None` for anything else.
    cue: Option<CueLive>,
    /// CUE-40, IPC-28: the effective BPM of a chase or BPM master.
    bpm: Option<f32>,
}

/// CUE-40: `ListLive` on the wire.
#[derive(Serialize, Clone, Copy, PartialEq)]
struct CueLive {
    current: Option<CueNumber>,
    next: Option<CueNumber>,
    progress: f32,
    paused: bool,
}

#[derive(Serialize)]
struct PlaybackEntry<'a> {
    #[serde(flatten)]
    stored: &'a StoredInfo,
    #[serde(flatten)]
    live: LiveInfo,
}

/// PBK-35: the pages' legends.
#[derive(Serialize, Clone, PartialEq)]
struct PageLegendInfo {
    number: u16,
    legend: String,
}

/// PBK-35: the cue and release settings of PBK-12 and PBK-20.
#[derive(Serialize, Clone, PartialEq)]
struct PlaybackSettingsInfo {
    record_by: RecordBy,
    delay: f32,
    fade_in: f32,
    fade_out: f32,
    overlap: f32,
    release_mask: Vec<AttributeGroup>,
    release_time: f32,
    auto_connect: bool,
    cue_list_fader: ListFader,
    update_mode: UpdateMode,
    handle_buttons: HandleButtons,
}

fn playback_settings(engine: &Engine) -> PlaybackSettingsInfo {
    let s = engine.settings();
    PlaybackSettingsInfo {
        record_by: s.cue_record_by,
        delay: s.cue_delay,
        fade_in: s.cue_fade_in,
        fade_out: s.cue_fade_out,
        overlap: s.cue_overlap,
        release_mask: s.release_mask.clone(),
        release_time: s.release_time,
        auto_connect: s.auto_connect,
        cue_list_fader: s.cue_list_fader,
        update_mode: s.update_mode,
        handle_buttons: s.handle_buttons.clone(),
    }
}

fn page_legends(engine: &Engine) -> Vec<PageLegendInfo> {
    (1..=PAGES)
        .map(|number| PageLegendInfo {
            number,
            legend: engine.page_legend(number).unwrap_or_default().to_string(),
        })
        .collect()
}

/// PBK-35, sorted by id.
fn stored_infos(engine: &Engine) -> Vec<StoredInfo> {
    engine
        .playbacks()
        .map(|p| {
            let times = p.cues.first().map_or(
                TimesInfo {
                    delay: 0.0,
                    fade_in: 0.0,
                    fade_out: 0.0,
                    overlap: 100.0,
                },
                |c| TimesInfo {
                    delay: c.delay,
                    fade_in: c.fade_in,
                    fade_out: c.fade_out,
                    overlap: c.overlap,
                },
            );
            StoredInfo {
                id: p.id,
                page: p.page,
                slot: p.slot,
                legend: p.legend.clone(),
                icon: p.icon.clone(),
                auto_icon: engine.playback_auto_icon(p.id),
                halo: engine.playback_halo(p.id),
                options: OptionsInfo {
                    priority: p.priority,
                    fader_mode: p.fader_mode,
                    kill_point: p.kill_point,
                    release_mask: p.release_mask.clone(),
                    release_time: p.release_time,
                    paging: p.paging,
                    group: p.group,
                },
                times,
                groups: engine.playback_groups(p.id),
                kind: p.kind,
                cues: (p.cues.iter())
                    .map(|c| CueInfo {
                        number: c.number,
                        legend: c.legend.clone(),
                        delay: c.delay,
                        fade_in: c.fade_in,
                        fade_out: c.fade_out,
                        overlap: c.overlap,
                        link: c.link,
                        track: c.track,
                        group_times: c.group_times.clone(),
                        groups: engine.cue_groups(p.id, c.number),
                    })
                    .collect(),
                list: p.list.clone(),
                chase: p.chase.clone(),
                master: p.master.clone(),
                buttons: engine
                    .handle_buttons(p.id)
                    .unwrap_or([Button::None, Button::None]),
            }
        })
        .collect()
}

/// IPC-29: whether anything stored changed, not counting the `level` of an enabled group master
/// (it follows the fader, which has its own small delta).
fn stored_changed(old: &[StoredInfo], new: &[StoredInfo]) -> bool {
    let same = |a: &StoredInfo, b: &StoredInfo| {
        a == b
            || match (&a.master, &b.master) {
                (Some(Master::Group(x)), Some(Master::Group(y))) if x.enabled && y.enabled => {
                    let level = GroupMaster {
                        level: y.level,
                        ..x.clone()
                    };
                    let a = StoredInfo {
                        master: Some(Master::Group(level)),
                        ..a.clone()
                    };
                    a == *b
                }
                _ => false,
            }
    };
    old.len() != new.len() || !old.iter().zip(new).all(|(a, b)| same(a, b))
}

fn live_info(engine: &Engine, id: PlaybackId) -> LiveInfo {
    let cue = engine.list_live(id).map(|l| CueLive {
        current: l.current,
        next: l.next,
        progress: l.progress,
        paused: l.paused,
    });
    let bpm = engine.effective_bpm(id);
    let Some(s) = engine.playback_state(id) else {
        return LiveInfo {
            level: 0.0,
            state: "off",
            flash: false,
            latched: false,
            fired: None,
            cue,
            bpm,
        };
    };
    LiveInfo {
        level: s.level,
        state: if s.is_active() {
            "active"
        } else if s.is_releasing() {
            "releasing"
        } else {
            "off"
        },
        flash: s.flash || s.latched,
        latched: s.latched,
        fired: s.activated,
        cue,
        bpm,
    }
}

/// PBK-03: the current page and what its slots hold.
#[derive(Serialize, Clone, PartialEq)]
struct PageInfo {
    number: u16,
    count: u16,
    slots: Vec<SlotInfo>,
}

#[derive(Serialize, Clone, PartialEq)]
struct SlotInfo {
    /// 1-based.
    slot: u16,
    playback: Option<PlaybackId>,
}

fn page_info(engine: &Engine) -> PageInfo {
    PageInfo {
        number: engine.page(),
        count: PAGES,
        slots: (1..)
            .zip(engine.page_slots())
            .map(|(slot, playback)| SlotInfo { slot, playback })
            .collect(),
    }
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
    /// IPC-21: attribute name to group, for placing programmer values.
    attributes: BTreeMap<&'a str, AttributeGroup>,
    /// PAT-09: every mode name of the fixture's profile, in profile order.
    modes: Vec<&'a str>,
}

#[derive(Serialize)]
struct GroupInfo<'a> {
    id: GroupId,
    fixtures: &'a [FixtureId],
}

/// PAL-25: one palette as clients draw it.
#[derive(Serialize, Clone, PartialEq)]
struct PaletteInfo {
    id: PaletteId,
    number: u32,
    window: cmdmx_core::palette::Window,
    handle: u16,
    legend: String,
    icon: Option<String>,
    auto_icon: Option<AutoIcon>,
    kind: PaletteKind,
    groups: Vec<AttributeGroup>,
}

/// PAL-25: the master time (s) and overlap (%) and the default Record By, as clients show them.
#[derive(Serialize, Clone, PartialEq)]
struct PaletteSettings {
    time: f32,
    overlap: f32,
    record_by: RecordBy,
}

fn palette_settings(engine: &Engine) -> PaletteSettings {
    let s = engine.settings();
    PaletteSettings {
        time: s.palette_time,
        overlap: s.palette_overlap,
        record_by: s.record_by,
    }
}

/// IPC-33: the programmer settings (PRG-42), as clients show them.
#[derive(Serialize, Clone, PartialEq)]
struct ProgrammerSettings {
    clear_precedence: ClearPrecedence,
    clear_ltp: ClearLtp,
    lowlight_level: f32,
}

fn programmer_settings(engine: &Engine) -> ProgrammerSettings {
    let s = engine.settings();
    ProgrammerSettings {
        clear_precedence: s.clear_precedence,
        clear_ltp: s.clear_ltp,
        lowlight_level: s.lowlight_level,
    }
}

#[derive(Serialize)]
struct ProgrammerSettingsDelta {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    settings: ProgrammerSettings,
}

#[derive(Serialize)]
struct PaletteSettingsDelta {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    settings: PaletteSettings,
}

#[derive(Serialize)]
struct PalettesDelta<'a> {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    palettes: &'a [PaletteInfo],
}

/// PAL-25: sorted by number.
fn palette_infos(engine: &Engine) -> Vec<PaletteInfo> {
    let mut list: Vec<PaletteInfo> = engine
        .palettes()
        .map(|p| PaletteInfo {
            id: p.id,
            number: p.number,
            window: p.window,
            handle: p.handle,
            legend: p.legend.clone(),
            icon: p.icon.clone(),
            auto_icon: engine.palette_auto_icon(p.number),
            kind: p.kind(),
            groups: engine.palette_groups(p.number),
        })
        .collect();
    list.sort_by_key(|p| p.number);
    list
}

#[derive(Serialize)]
struct Snapshot<'a> {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    faders_per_page: u16,
    grand_master: f32,
    page: PageInfo,
    playbacks: Vec<PlaybackEntry<'a>>,
    /// CUE-26, CUE-40: the connected list.
    connected: Option<PlaybackId>,
    pages: Vec<PageLegendInfo>,
    dbo: bool,
    playback_settings: PlaybackSettingsInfo,
    fixtures: Vec<FixtureInfo<'a>>,
    groups: Vec<GroupInfo<'a>>,
    palettes: Vec<PaletteInfo>,
    palette_settings: PaletteSettings,
    programmer_settings: ProgrammerSettings,
    programmer: &'a ProgrammerView,
    outputs: Vec<OutputJson>,
    show: &'a ShowState,
    undo: &'a UndoInfo,
    /// CTL-39
    control: &'a serde_json::Value,
}

/// OUT-33: one output node with its fields, state and detail.
#[derive(Serialize)]
struct OutputJson {
    #[serde(flatten)]
    node: Node,
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

impl From<&OutputStatus> for OutputJson {
    fn from(o: &OutputStatus) -> Self {
        let (state, detail) = match o.state {
            OutputState::Ok => ("Ok", o.detail.clone()),
            OutputState::Degraded => ("Degraded", o.detail.clone()),
            OutputState::NoInterface => ("Degraded", Some("no interface".into())),
            OutputState::PortNotFound => ("Degraded", Some("port not found".into())),
            OutputState::Stopped => ("Stopped", o.detail.clone()),
            OutputState::Disabled => ("Disabled", o.detail.clone()),
        };
        OutputJson {
            node: o.node.clone(),
            state,
            detail,
        }
    }
}

fn output_jsons(outputs: &[OutputStatus]) -> Vec<OutputJson> {
    outputs.iter().map(OutputJson::from).collect()
}

#[derive(Serialize)]
struct ShowDelta<'a> {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    state: &'a ShowState,
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
    id: PlaybackId,
    #[serde(flatten)]
    live: LiveInfo,
}

#[derive(Serialize)]
struct ConnectedDelta {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    id: Option<PlaybackId>,
}

#[derive(Serialize)]
struct PlaybacksDelta<'a> {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    playbacks: Vec<PlaybackEntry<'a>>,
}

#[derive(Serialize)]
struct PagesDelta<'a> {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    pages: &'a [PageLegendInfo],
}

#[derive(Serialize)]
struct DboDelta {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    on: bool,
}

#[derive(Serialize)]
struct PlaybackSettingsDelta<'a> {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    settings: &'a PlaybackSettingsInfo,
}

#[derive(Serialize)]
struct GrandMasterDelta {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    level: f32,
}

#[derive(Serialize)]
struct PageDelta {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    page: PageInfo,
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

/// The whole playback list, stored part and live part together.
fn entries<'a>(stored: &'a [StoredInfo], engine: &Engine) -> Vec<PlaybackEntry<'a>> {
    stored
        .iter()
        .map(|s| PlaybackEntry {
            live: live_info(engine, s.id),
            stored: s,
        })
        .collect()
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
                attributes: (mode.iter())
                    .flat_map(|m| m.logical_attributes())
                    .map(|l| (l.name, l.group))
                    .collect(),
                modes: (engine.profile(&f.profile).into_iter())
                    .flat_map(|p| p.modes.iter().map(|m| m.name.as_str()))
                    .collect(),
            }
        })
        .collect()
}

/// IPC-03/10/20: full state for a freshly connected client.
pub fn snapshot(
    engine: &Engine,
    outputs: &[OutputStatus],
    show: &ShowState,
    undo: &UndoInfo,
    control: &serde_json::Value,
) -> String {
    let stored = stored_infos(engine);
    to_json(&Snapshot {
        v: 0,
        kind: "snapshot",
        faders_per_page: engine.settings().faders_per_page,
        grand_master: engine.grand_master(),
        page: page_info(engine),
        playbacks: entries(&stored, engine),
        connected: engine.connected(),
        pages: page_legends(engine),
        dbo: engine.dbo(),
        playback_settings: playback_settings(engine),
        fixtures: fixture_infos(engine),
        groups: group_infos(engine.groups()),
        palettes: palette_infos(engine),
        palette_settings: palette_settings(engine),
        programmer_settings: programmer_settings(engine),
        programmer: &engine.programmer_view(),
        outputs: output_jsons(outputs),
        show,
        undo,
        control,
    })
}

/// IPC-24: one value of a cue's state.
#[derive(Serialize)]
struct CueViewValue {
    value: Value,
    from: CueNumber,
    /// Only for a palette reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    palette: Option<PaletteId>,
}

#[derive(Serialize)]
struct CueViewFixture {
    fixture: FixtureId,
    values: BTreeMap<String, CueViewValue>,
}

#[derive(Serialize)]
struct CueViewMessage {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    playback: PlaybackId,
    cue: CueNumber,
    fixtures: Vec<CueViewFixture>,
}

/// CUE-54, IPC-24: the answer to `cue.view`, sorted by fixture; a palette reference shows what
/// its palette resolves (as `contents`, IPC-21) with its id.
pub fn cue_view(engine: &Engine, id: PlaybackId, cue: CueNumber) -> Result<String, CoreError> {
    let fixtures = (engine.cue_view(id, cue)?.into_iter())
        .map(|(fixture, attributes)| CueViewFixture {
            fixture,
            values: (attributes.into_iter())
                .map(|(name, v)| {
                    let palette = match v.value {
                        Stored::Reference(r) => Some(r.palette),
                        Stored::Value(_) => None,
                    };
                    let value = engine.resolve_stored(fixture, &name, v.value);
                    let shown = CueViewValue {
                        value,
                        from: v.from,
                        palette,
                    };
                    (name, shown)
                })
                .collect(),
        })
        .collect();
    Ok(to_json(&CueViewMessage {
        v: 0,
        kind: "cue.view",
        playback: id,
        cue,
        fixtures,
    }))
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
    stored: Vec<StoredInfo>,
    live: BTreeMap<PlaybackId, LiveInfo>,
    connected: Option<PlaybackId>,
    grand_master: f32,
    page: PageInfo,
    pages: Vec<PageLegendInfo>,
    dbo: bool,
    playback_settings: PlaybackSettingsInfo,
    programmer: ProgrammerView,
    groups: BTreeMap<GroupId, Vec<FixtureId>>,
    palettes: Vec<PaletteInfo>,
    palette_settings: PaletteSettings,
    programmer_settings: ProgrammerSettings,
    fixtures: Vec<Fixture>,
    outputs: Vec<OutputStatus>,
    show: ShowState,
    undo: UndoInfo,
}

impl Published {
    pub fn new(engine: &Engine, outputs: &[OutputStatus], show: &ShowState) -> Self {
        Self {
            stored: stored_infos(engine),
            live: engine
                .playbacks()
                .map(|p| (p.id, live_info(engine, p.id)))
                .collect(),
            connected: engine.connected(),
            grand_master: engine.grand_master(),
            page: page_info(engine),
            pages: page_legends(engine),
            dbo: engine.dbo(),
            playback_settings: playback_settings(engine),
            programmer: engine.programmer_view(),
            groups: engine.groups().clone(),
            palettes: palette_infos(engine),
            palette_settings: palette_settings(engine),
            programmer_settings: programmer_settings(engine),
            fixtures: engine.fixtures().cloned().collect(),
            outputs: outputs.to_vec(),
            show: show.clone(),
            undo: UndoInfo::default(),
        }
    }

    /// IPC-31: a delta only when the labels differ from the last ones published.
    pub fn undo_delta(&mut self, info: &UndoInfo) -> Option<String> {
        if self.undo == *info {
            return None;
        }
        self.undo = info.clone();
        Some(to_json(&serde_json::json!({
            "v": 0,
            "type": "undo",
            "undo": info.undo,
            "redo": info.redo,
        })))
    }

    pub fn diff(
        &mut self,
        engine: &Engine,
        outputs: &[OutputStatus],
        show: &ShowState,
    ) -> Vec<String> {
        let mut out = Vec::new();
        // PBK-35: one whole-list delta after any stored change (it carries the live values
        // too), else a small delta per playback whose live part changed.
        let stored = stored_infos(engine);
        if stored_changed(&self.stored, &stored) {
            out.push(to_json(&PlaybacksDelta {
                v: 0,
                kind: "playbacks",
                playbacks: entries(&stored, engine),
            }));
            self.live = stored
                .iter()
                .map(|s| (s.id, live_info(engine, s.id)))
                .collect();
            self.stored = stored;
        } else {
            for s in &stored {
                let now = live_info(engine, s.id);
                if self.live.insert(s.id, now) != Some(now) {
                    out.push(to_json(&PlaybackDelta {
                        v: 0,
                        kind: "playback",
                        id: s.id,
                        live: now,
                    }));
                }
            }
            self.stored = stored;
        }
        // CUE-26, 40: the connected list, once, when it changes.
        let connected = engine.connected();
        if connected != self.connected {
            self.connected = connected;
            out.push(to_json(&ConnectedDelta {
                v: 0,
                kind: "connected",
                id: connected,
            }));
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
        // PBK-03: the whole page, once, when the number or a slot's playback changed.
        let page = page_info(engine);
        if page != self.page {
            self.page = page.clone();
            out.push(to_json(&PageDelta {
                v: 0,
                kind: "page",
                page,
            }));
        }
        let pages = page_legends(engine);
        if pages != self.pages {
            out.push(to_json(&PagesDelta {
                v: 0,
                kind: "pages",
                pages: &pages,
            }));
            self.pages = pages;
        }
        if engine.dbo() != self.dbo {
            self.dbo = engine.dbo();
            out.push(to_json(&DboDelta {
                v: 0,
                kind: "dbo",
                on: self.dbo,
            }));
        }
        let settings = playback_settings(engine);
        if settings != self.playback_settings {
            out.push(to_json(&PlaybackSettingsDelta {
                v: 0,
                kind: "playback_settings",
                settings: &settings,
            }));
            self.playback_settings = settings;
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
        // PAL-25: the whole list, once, after any change of it.
        let palettes = palette_infos(engine);
        if palettes != self.palettes {
            out.push(to_json(&PalettesDelta {
                v: 0,
                kind: "palettes",
                palettes: &palettes,
            }));
            self.palettes = palettes;
        }
        // PAL-25: the settings, once, after any change of them.
        let settings = palette_settings(engine);
        if settings != self.palette_settings {
            out.push(to_json(&PaletteSettingsDelta {
                v: 0,
                kind: "palette_settings",
                settings: settings.clone(),
            }));
            self.palette_settings = settings;
        }
        // IPC-33: the programmer settings, once, after any change of them.
        let settings = programmer_settings(engine);
        if settings != self.programmer_settings {
            out.push(to_json(&ProgrammerSettingsDelta {
                v: 0,
                kind: "programmer_settings",
                settings: settings.clone(),
            }));
            self.programmer_settings = settings;
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
        // SHW-15: the whole state, once, after any change of it.
        if show != &self.show {
            self.show = show.clone();
            out.push(to_json(&ShowDelta {
                v: 0,
                kind: "show",
                state: show,
            }));
        }
        out
    }
}

/// Outgoing queues of all connected clients, owned by the tick thread.
#[derive(Default)]
pub struct Clients {
    queues: Vec<SyncSender<String>>,
    /// The last warning sent, for OSC `/cmdmx/message` (CTL-37).
    pub last_warning: String,
}

impl Clients {
    /// Queue the snapshot first; a client that cannot even take that is not kept.
    pub fn add(&mut self, tx: SyncSender<String>, snapshot: &str) {
        if tx.try_send(snapshot.to_string()).is_ok() {
            self.queues.push(tx);
        }
    }

    /// Never blocks: a client whose queue is full or closed is dropped (IPC-07).
    pub fn broadcast(&mut self, msg: &str) {
        self.queues
            .retain(|tx| tx.try_send(msg.to_string()).is_ok());
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.queues.len()
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

/// SHW-15: the reply to `show.list`, to the asking client only.
pub fn answer_show_list(shows: &Shows) -> String {
    serde_json::json!({"v": 0, "type": "show.list", "shows": shows.list()}).to_string()
}

/// SHW-15: what a show request becomes. Names are checked here (SHW-09) and an Open is read
/// here, so the tick only switches. A refusal is a warning for every client (IPC-06).
fn show_event(shows: &Shows, req: ShowRequest) -> Event {
    match &req {
        ShowRequest::Open { name } => match valid_name(name).and_then(|()| shows.open(name)) {
            Ok(opened) => Event::ShowOpened(Box::new(opened)),
            Err(e) => Event::Warning(e),
        },
        ShowRequest::New { name } | ShowRequest::SaveAs { name } => {
            match valid_name(name).and_then(|()| match shows.exists(name) {
                true => Err(format!("a show named {name} already exists")),
                false => Ok(()),
            }) {
                Ok(()) => Event::Command(Command::Show(req)),
                Err(e) => Event::Warning(e),
            }
        }
        ShowRequest::List => Event::Command(Command::Show(req)),
    }
}

/// A server for tests that never use the library: it stays "loading". Its shows folder is a
/// temp folder nobody writes to.
#[cfg(test)]
pub fn spawn_server(port: u16) -> std::io::Result<(Receiver<Event>, SocketAddr)> {
    let dir = std::env::temp_dir().join(format!("cmdmx-ipc-shows-{}", std::process::id()));
    let shows = Shows::new(dir.join("Shows"), dir.join("state.json"));
    spawn_server_with(
        port,
        Arc::new(RwLock::new(None)),
        Arc::new(shows),
        Instant::now(),
    )
}

/// IPC-01: listen on loopback (port 0 = ephemeral); events flow to the tick, library
/// requests are answered by the client threads from `library` (LIB-14) and the show requests
/// from `shows` (SHW-15). `started` is the tick's clock: taps and buttons are stamped with the
/// seconds since it when a client thread receives them (IPC-25).
pub fn spawn_server_with(
    port: u16,
    library: SharedLibrary,
    shows: Arc<Shows>,
    started: Instant,
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
                let shows = Arc::clone(&shows);
                // A failed spawn only loses this one client.
                let _ = std::thread::Builder::new()
                    .name("ipc-client".into())
                    .spawn(move || {
                        // IPC-07: a panic closes only this client.
                        if catch_unwind(AssertUnwindSafe(|| {
                            serve_client(stream, &events, &library, &shows, started)
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

fn serve_client(
    stream: TcpStream,
    events: &Sender<Event>,
    library: &RwLock<Option<Library>>,
    shows: &Shows,
    started: Instant,
) {
    let Ok(mut ws) = tungstenite::accept_hdr(stream, refuse_origin) else {
        return;
    };
    let (tx, out) = sync_channel::<String>(QUEUE);
    if events.send(Event::Connected(tx)).is_err() {
        return;
    }
    // IPC-24: the tick answers `cue.view` into this queue. It is not `tx`, because a sender kept
    // here would stop `out` from reporting that the tick dropped this client (IPC-07).
    let (reply, replies) = sync_channel::<String>(QUEUE);
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
                Ok(Command::OutputPorts) => {
                    // An unreadable port list is an empty one; the tick is not involved.
                    let ports = serialport::available_ports().unwrap_or_default();
                    if ws.send(Message::text(ports_reply(&ports))).is_err() {
                        return;
                    }
                }
                Ok(Command::Show(ShowRequest::List)) => {
                    if ws.send(Message::text(answer_show_list(shows))).is_err() {
                        return;
                    }
                }
                Ok(Command::Show(req)) => {
                    if events.send(show_event(shows, req)).is_err() {
                        return;
                    }
                }
                Ok(Command::ControlMapping { name }) => {
                    let reply = reply.clone();
                    if events.send(Event::ControlMapping { name, reply }).is_err() {
                        return;
                    }
                }
                Ok(Command::CueView { id, cue }) => {
                    let reply = reply.clone();
                    if events.send(Event::CueView { id, cue, reply }).is_err() {
                        return;
                    }
                }
                Ok(cmd) => {
                    let cmd = stamp(cmd, started.elapsed().as_secs_f64());
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
        while let Ok(msg) = replies.try_recv() {
            if ws.send(Message::text(msg)).is_err() {
                return;
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
    use cmdmx_core::playback::Playback;
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
        e.add_playback(Playback::new(
            1,
            1,
            1,
            BTreeMap::from([(1, BTreeMap::from([("Dimmer".to_string(), 1.0.into())]))]),
        ));
        e
    }

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn ipc_03_snapshot_has_settings_grand_master_and_playbacks() {
        let v = parse(&snapshot(
            &engine(),
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert_eq!(v["v"], 0);
        assert_eq!(v["type"], "snapshot");
        assert_eq!(v["faders_per_page"], 8);
        assert_eq!(v["grand_master"], 1.0);
        assert_eq!(v["playbacks"], json!([playback_entry()]));
    }

    /// OUT-33: one node per state, sorted by line then id as the reconciler keeps them.
    fn outputs() -> Vec<OutputStatus> {
        use cmdmx_core::output::{Node, NodeKind};
        let node = |id, line, kind| Node {
            id,
            line,
            enabled: true,
            kind,
        };
        let status = |node, state, detail: Option<&str>| OutputStatus {
            node,
            state,
            detail: detail.map(String::from),
        };
        let mut off = node(5, 2, NodeKind::default_artnet(2));
        off.enabled = false;
        vec![
            status(
                node(1, 1, NodeKind::default_artnet(1)),
                OutputState::Ok,
                None,
            ),
            status(
                node(2, 1, NodeKind::default_sacn(1)),
                OutputState::Degraded,
                Some("no route to host"),
            ),
            status(
                node(
                    3,
                    1,
                    NodeKind::Serial {
                        port: "auto".into(),
                    },
                ),
                OutputState::NoInterface,
                None,
            ),
            status(
                node(
                    4,
                    1,
                    NodeKind::Serial {
                        port: "/dev/cu.usbserial-A".into(),
                    },
                ),
                OutputState::PortNotFound,
                None,
            ),
            status(off, OutputState::Disabled, None),
            status(
                node(6, 2, NodeKind::default_sacn(2)),
                OutputState::Disabled,
                Some("--no-sacn"),
            ),
            status(
                node(7, 3, NodeKind::default_sacn(3)),
                OutputState::Stopped,
                None,
            ),
        ]
    }

    #[test]
    fn out_33_snapshot_lists_every_node_with_its_fields_and_state() {
        let v = parse(&snapshot(
            &engine(),
            &outputs(),
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert_eq!(
            v["outputs"],
            json!([
                {"id":1,"line":1,"enabled":true,"kind":"artnet","port_address":0,
                 "target":"255.255.255.255","state":"Ok"},
                {"id":2,"line":1,"enabled":true,"kind":"sacn","universe":1,"priority":100,
                 "unicast":null,"state":"Degraded","detail":"no route to host"},
                {"id":3,"line":1,"enabled":true,"kind":"serial","port":"auto",
                 "state":"Degraded","detail":"no interface"},
                {"id":4,"line":1,"enabled":true,"kind":"serial","port":"/dev/cu.usbserial-A",
                 "state":"Degraded","detail":"port not found"},
                {"id":5,"line":2,"enabled":false,"kind":"artnet","port_address":1,
                 "target":"255.255.255.255","state":"Disabled"},
                {"id":6,"line":2,"enabled":true,"kind":"sacn","universe":2,"priority":100,
                 "unicast":null,"state":"Disabled","detail":"--no-sacn"},
                {"id":7,"line":3,"enabled":true,"kind":"sacn","universe":3,"priority":100,
                 "unicast":null,"state":"Stopped"}
            ])
        );
        assert_eq!(
            parse(&snapshot(
                &engine(),
                &[],
                &show_state(),
                &UndoInfo::default(),
                &serde_json::Value::Null,
            ))["outputs"],
            json!([])
        );
    }

    #[test]
    fn out_33_outputs_delta_when_a_state_or_a_node_changes() {
        let e = engine();
        let mut outs = outputs();
        let mut published = Published::new(&e, &outs, &show_state());
        assert!(published.diff(&e, &outs, &show_state()).is_empty());
        outs[0].state = OutputState::Stopped;
        let d = published.diff(&e, &outs, &show_state());
        assert_eq!(d.len(), 1);
        let v = parse(&d[0]);
        assert_eq!(v["v"], 0);
        assert_eq!(v["type"], "outputs");
        assert_eq!(v["outputs"][0]["state"], "Stopped");
        assert_eq!(v["outputs"].as_array().unwrap().len(), 7);
        assert!(
            published.diff(&e, &outs, &show_state()).is_empty(),
            "no change, no delta"
        );
        // An edited node (here its line) is a change too; so is a removed one.
        outs[1].node.line = 4;
        assert_eq!(published.diff(&e, &outs, &show_state()).len(), 1);
        outs.pop();
        let d = published.diff(&e, &outs, &show_state());
        assert_eq!(parse(&d[0])["outputs"].as_array().unwrap().len(), 6);
    }

    #[test]
    fn out_34_output_commands_parse_with_the_lines_defaults() {
        use cmdmx_core::output::NodeKind;
        let ok = |s: &str| parse_command(s).unwrap_or_else(|e| panic!("{s}: {e}"));
        assert_eq!(
            ok(r#"{"v":0,"cmd":"output.add","line":2,"kind":"artnet"}"#),
            Command::OutputAdd {
                line: 2,
                enabled: true,
                kind: NodeKind::default_artnet(2),
            }
        );
        assert_eq!(
            ok(
                r#"{"v":0,"cmd":"output.add","line":3,"kind":"sacn","priority":150,"unicast":"10.0.0.5","enabled":false}"#
            ),
            Command::OutputAdd {
                line: 3,
                enabled: false,
                kind: NodeKind::Sacn {
                    universe: 3,
                    priority: 150,
                    unicast: Some("10.0.0.5".into()),
                },
            }
        );
        assert_eq!(
            ok(
                r#"{"v":0,"cmd":"output.add","line":1,"kind":"artnet","port_address":17,"target":"2.255.255.255"}"#
            ),
            Command::OutputAdd {
                line: 1,
                enabled: true,
                kind: NodeKind::Artnet {
                    port_address: 17,
                    target: "2.255.255.255".into(),
                },
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"output.add","line":2,"kind":"serial"}"#),
            Command::OutputAdd {
                line: 2,
                enabled: true,
                kind: NodeKind::Serial {
                    port: "auto".into()
                },
            }
        );
        let Command::OutputEdit { id, change } =
            ok(r#"{"v":0,"cmd":"output.edit","id":3,"target":"10.0.0.9","enabled":false}"#)
        else {
            panic!("not an edit");
        };
        assert_eq!(id, 3);
        assert_eq!(
            serde_json::Value::Object(change),
            json!({"target":"10.0.0.9","enabled":false}),
            "only the fields to change; cmd, v and id are not among them"
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"output.remove","ids":[3,4]}"#),
            Command::OutputRemove { ids: vec![3, 4] }
        );
        assert_eq!(ok(r#"{"v":0,"cmd":"output.ports"}"#), Command::OutputPorts);
        for bad in [
            r#"{"v":0,"cmd":"output.add","kind":"artnet"}"#,
            r#"{"v":0,"cmd":"output.add","line":1}"#,
            r#"{"v":0,"cmd":"output.add","line":1,"kind":"dmxking"}"#,
            r#"{"v":0,"cmd":"output.add","line":1,"kind":"sacn","priority":"high"}"#,
            r#"{"v":0,"cmd":"output.add","line":1,"kind":"artnet","port_address":-1}"#,
            r#"{"v":0,"cmd":"output.edit","target":"10.0.0.9"}"#,
            r#"{"v":0,"cmd":"output.remove","ids":3}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn out_28_output_add_numbers_too_wide_get_the_range_text() {
        for (fields, text) in [
            (
                r#""kind":"sacn","priority":300"#,
                "sACN priority 300 is out of range (0 to 200)",
            ),
            (
                r#""kind":"sacn","universe":70000"#,
                "sACN universe 70000 is out of range (1 to 63999)",
            ),
            (
                r#""kind":"artnet","port_address":70000"#,
                "Art-Net port-address 70000 is out of range (0 to 32767)",
            ),
            (
                r#""kind":"artnet","port_address":-1"#,
                "Art-Net port-address -1 is out of range (0 to 32767)",
            ),
        ] {
            let add = format!(r#"{{"v":0,"cmd":"output.add","line":2,{fields}}}"#);
            assert_eq!(parse_command(&add), Err(text.to_string()), "{add}");
        }
        assert_eq!(
            parse_command(r#"{"v":0,"cmd":"output.add","line":70000,"kind":"serial"}"#),
            Err("universe 70000 is out of range (1 to 65535)".to_string())
        );
    }

    #[test]
    fn out_35_ports_answer_lists_cu_ports_sorted_by_path() {
        use serialport::{SerialPortInfo, SerialPortType, UsbPortInfo};
        let usb = |path: &str, vid, pid, product: &str| SerialPortInfo {
            port_name: path.into(),
            port_type: SerialPortType::UsbPort(UsbPortInfo {
                vid,
                pid,
                serial_number: None,
                manufacturer: None,
                product: Some(product.into()),
            }),
        };
        let ports = [
            usb("/dev/cu.usbserial-Z9", 0x0403, 0x6001, "DMX USB PRO"),
            usb(
                "/dev/tty.usbserial-A10KXYZ",
                0x0403,
                0x6001,
                "FT232R USB UART",
            ),
            usb(
                "/dev/cu.usbserial-A10KXYZ",
                0x0403,
                0x6001,
                "FT232R USB UART",
            ),
            SerialPortInfo {
                port_name: "/dev/cu.Bluetooth-Incoming-Port".into(),
                port_type: SerialPortType::Unknown,
            },
        ];
        assert_eq!(
            parse(&ports_reply(&ports)),
            json!({"v":0,"type":"ports","ports":[
                {"path":"/dev/cu.Bluetooth-Incoming-Port","product":null,"vid":null,"pid":null,
                 "enttec":false},
                {"path":"/dev/cu.usbserial-A10KXYZ","product":"FT232R USB UART","vid":"0403",
                 "pid":"6001","enttec":false},
                {"path":"/dev/cu.usbserial-Z9","product":"DMX USB PRO","vid":"0403",
                 "pid":"6001","enttec":true}
            ]})
        );
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
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert_eq!(v["fixtures"], json!([dimmer_entry()]));
        assert_eq!(v["groups"], json!([{"id":1,"fixtures":[1]}]));
        assert_eq!(
            v["programmer"],
            json!({"selection":[],"targets":[],"active_group":"I",
                   "attribute_groups": no_values(false),"wheels":[],"in_programmer":[],"colour_model":"Hsi",
                   "contents":[],"colour":null,"palettes":{"in_use":[],"relevant":[]},
                   "located":{},"highlight":false})
        );
    }

    /// PAT-09: the test engine's one dimmer as a snapshot entry.
    fn dimmer_entry() -> Value {
        json!({"id":1,"number":1,"name":"","profile":"dimmer","mode":"default","universe":1,
               "address":1,"footprint":1,"virtual_dimmer":false,"handle":{"page":1,"slot":1},
               "attributes":{"Dimmer":"I"},"modes":["default"]})
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
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert_eq!(
            v["fixtures"],
            json!([dimmer_entry(),
                   {"id":2,"number":221,"name":"Front left","profile":"Showtec/LED Par 56",
                    "mode":"6 Channel","universe":1,"address":37,"footprint":6,
                    "virtual_dimmer":true,"handle":{"page":1,"slot":2},
                    "attributes":{"Dimmer":"I","Colour":"C","Mode":"S","Speed":"S","Strobe":"I"},
                    "modes":["6 Channel"]}])
        );
    }

    /// PAT-09 (amended, VS2-16): every mode of the show's profile, in profile order, whatever
    /// mode the fixture is in.
    #[test]
    fn pat_09_entries_carry_the_profile_modes() {
        let two_modes = r#"{"manufacturer":"Acme","model":"Wash","source":"channel-list","modes":[
            {"name":"16 Channel","footprint":16,"attributes":[{"name":"Dimmer","group":"I","kind":"Htp","channel":1}]},
            {"name":"24 Channel","footprint":24,"attributes":[{"name":"Dimmer","group":"I","kind":"Htp","channel":1}]}]}"#;
        let mut e = engine();
        e.add_profile("Acme/Wash", Profile::from_json(two_modes).unwrap())
            .unwrap();
        e.patch_add(&cmdmx_core::patch::PatchAdd {
            profile: "Acme/Wash".into(),
            mode: Some("24 Channel".into()),
            count: 1,
            universe: 1,
            address: Some(101),
            number: Some(901),
        })
        .unwrap();
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        let wash = &v["fixtures"][1];
        assert_eq!(wash["mode"], "24 Channel");
        assert_eq!(wash["modes"], json!(["16 Channel", "24 Channel"]));
    }

    #[test]
    fn pat_09_fixtures_delta_after_a_patch_change_only() {
        let mut e = engine();
        let mut published = Published::new(&e, &[], &show_state());
        e.tick(0.0);
        assert!(published.diff(&e, &[], &show_state()).is_empty());
        e.patch_renumber(1, 42).unwrap();
        e.tick(0.025);
        let d: Vec<Value> = published
            .diff(&e, &[], &show_state())
            .iter()
            .map(|m| parse(m))
            .filter(|m| m["type"] == "fixtures")
            .collect();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0]["v"], 0);
        assert_eq!(d[0]["fixtures"][0]["number"], 42);
        e.tick(0.05);
        assert!(
            published.diff(&e, &[], &show_state()).is_empty(),
            "no change, no delta"
        );
        e.patch_move(1, 2, 100).unwrap();
        let d = published.diff(&e, &[], &show_state());
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
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert_eq!(
            v["programmer"],
            json!({"selection":[1],"targets":[1],"active_group":"I",
                   "attribute_groups": no_values(true),
                   "wheels":[{"attribute":"Dimmer","value":0.0,"touched":false,"sixteen_bit":false,"palette":null}],
                   "in_programmer":[],"colour_model":"Hsi","contents":[],"colour":null,
                   "palettes":{"in_use":[],"relevant":[]},
                   "located":{},"highlight":false})
        );
        e.turn_wheel("Dimmer", 255, Resolution::Normal).unwrap();
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert_eq!(
            v["programmer"]["wheels"],
            json!([{"attribute":"Dimmer","value":1.0,"touched":true,"sixteen_bit":false,"palette":null}])
        );
        assert_eq!(v["programmer"]["in_programmer"], json!([1]));
        // IPC-21: programmer values per fixture.
        assert_eq!(
            v["programmer"]["contents"],
            json!([{"fixture":1,"values":{"Dimmer":1.0},"palettes":{},"fading":{}}])
        );
        assert_eq!(v["programmer"]["attribute_groups"][0]["has_values"], true);
    }

    #[test]
    fn ipc_12_programmer_and_groups_deltas_only_on_change() {
        let mut e = engine();
        let mut published = Published::new(&e, &[], &show_state());
        e.tick(0.0);
        assert!(published.diff(&e, &[], &show_state()).is_empty());

        e.select_fixture(1).unwrap();
        e.tick(0.025);
        let d = published.diff(&e, &[], &show_state());
        assert_eq!(d.len(), 1);
        let p = parse(&d[0]);
        assert_eq!(p["v"], 0);
        assert_eq!(p["type"], "programmer");
        assert_eq!(p["selection"], json!([1]));
        assert_eq!(p["wheels"][0]["attribute"], "Dimmer");

        e.tick(0.05);
        assert!(
            published.diff(&e, &[], &show_state()).is_empty(),
            "no change, no delta"
        );

        e.record_group(4).unwrap();
        e.tick(0.075);
        let d = published.diff(&e, &[], &show_state());
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
    fn ipc_23_colour_command_parses_and_checks_its_range() {
        assert_eq!(
            parse_command(r#"{"v":0,"cmd":"programmer.colour","hue":0.66,"saturation":1.0}"#),
            Ok(Command::Colour {
                hue: 0.66,
                saturation: 1.0
            })
        );
        for bad in [
            r#"{"v":0,"cmd":"programmer.colour","hue":0.66}"#,
            r#"{"v":0,"cmd":"programmer.colour","saturation":1.0}"#,
            r#"{"v":0,"cmd":"programmer.colour","hue":1.2,"saturation":1.0}"#,
            r#"{"v":0,"cmd":"programmer.colour","hue":0.5,"saturation":-0.1}"#,
            r#"{"v":0,"cmd":"programmer.colour","hue":"red","saturation":1.0}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn ipc_18_colour_model_change_is_a_programmer_delta() {
        let mut e = engine();
        let mut published = Published::new(&e, &[], &show_state());
        e.tick(0.0);
        assert!(published.diff(&e, &[], &show_state()).is_empty());
        e.set_colour_model(ColourModel::Cmy);
        e.tick(0.025);
        let d = published.diff(&e, &[], &show_state());
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
    fn ipc_35_manufacturers_answer_the_asking_client() {
        let lib = library("manufacturers");
        let v = ask(&lib, r#"{"v":0,"cmd":"library.manufacturers","query":"par"}"#);
        assert_eq!(
            v,
            json!({"v":0,"type":"library.manufacturers","query":"par","items":[
                {"name":"Pro-Lights","models":1},{"name":"Showtec","models":1}]})
        );
        let v = ask(&lib, r#"{"v":0,"cmd":"library.manufacturers"}"#);
        assert_eq!(v["type"], "library.manufacturers");
        assert_eq!(v["query"], Value::Null);
        assert_eq!(v["items"].as_array().unwrap().len(), 3);
        let loading = std::sync::RwLock::new(None);
        let v = ask(&loading, r#"{"v":0,"cmd":"library.manufacturers"}"#);
        assert_eq!(v["type"], "error");
        assert!(v["message"].as_str().unwrap().contains("loading"), "{v}");
    }

    #[test]
    fn ipc_36_models_answer_the_asking_client_and_echo_the_request() {
        let lib = library("models");
        let v = ask(
            &lib,
            r#"{"v":0,"cmd":"library.models","manufacturer":"Shehds","query":"19x15"}"#,
        );
        assert_eq!(
            v,
            json!({"v":0,"type":"library.models","manufacturer":"Shehds","query":"19x15",
                   "items":[{"manufacturer":"Shehds","model":"LED Beam+Wash 19x15W RGBW Zoom Lighting",
                             "modes":[{"name":"16 Channel","channels":16},{"name":"24 Channel","channels":24}],
                             "source":"qlcplus"}],
                   "truncated":false})
        );
        let v = ask(&lib, r#"{"v":0,"cmd":"library.models","manufacturer":"Showtec"}"#);
        assert_eq!(v["query"], Value::Null);
        assert_eq!(
            v["items"][0]["modes"],
            json!([{"name":"6 Channel","channels":6}])
        );
        let v = ask(&lib, r#"{"v":0,"cmd":"library.models","query":"par"}"#);
        assert_eq!(v["manufacturer"], Value::Null);
        assert_eq!(v["items"].as_array().unwrap().len(), 2);
        assert_eq!(v["truncated"], false);
        let loading = std::sync::RwLock::new(None);
        let v = ask(&loading, r#"{"v":0,"cmd":"library.models","query":"x"}"#);
        assert!(v["message"].as_str().unwrap().contains("loading"), "{v}");
    }

    #[test]
    fn ipc_36_models_without_manufacturer_or_query_is_an_error() {
        for bad in [
            r#"{"v":0,"cmd":"library.models"}"#,
            r#"{"v":0,"cmd":"library.models","manufacturer":3}"#,
            r#"{"v":0,"cmd":"library.manufacturers","query":3}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
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
        let mut published = Published::new(&e, &[], &show_state());
        e.tick(0.0);
        assert!(published.diff(&e, &[], &show_state()).is_empty());

        e.set_fader(1, 0.5, 0.025).unwrap();
        e.tick(0.025);
        let d = published.diff(&e, &[], &show_state());
        assert_eq!(d.len(), 1);
        let mut v = parse(&d[0]);
        assert!(v["fired"].is_u64(), "{v}");
        v.as_object_mut().unwrap().remove("fired");
        assert_eq!(
            v,
            json!({"v":0,"type":"playback","id":1,"level":0.5,"state":"active",
                   "flash":false,"latched":false,"cue":null,"bpm":null})
        );
        e.tick(0.05);
        assert!(
            published.diff(&e, &[], &show_state()).is_empty(),
            "no change, no delta"
        );

        e.set_grand_master(0.25).unwrap();
        let d = published.diff(&e, &[], &show_state());
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
        tx.send(snapshot(
            &engine(),
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ))
        .unwrap();
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
            _ => panic!("expected Command"),
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

    // ---- SHW-15 --------------------------------------------------------------

    fn show_state() -> crate::shows::ShowState {
        crate::shows::ShowState {
            name: "Kerk".into(),
            saved_at: Some("2026-10-18T09:00:05Z".into()),
            autosave_ok: true,
        }
    }

    #[test]
    fn shw_15_snapshot_has_the_show_state() {
        let v = parse(&snapshot(
            &engine(),
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert_eq!(
            v["show"],
            json!({"name":"Kerk","saved_at":"2026-10-18T09:00:05Z","autosave_ok":true})
        );
        let fresh = crate::shows::ShowState {
            saved_at: None,
            ..show_state()
        };
        assert_eq!(
            parse(&snapshot(
                &engine(),
                &[],
                &fresh,
                &UndoInfo::default(),
                &serde_json::Value::Null
            ))["show"]["saved_at"],
            Value::Null
        );
    }

    #[test]
    fn shw_15_show_delta_only_when_the_state_changes() {
        let e = engine();
        let mut state = show_state();
        let mut published = Published::new(&e, &[], &state);
        assert!(published.diff(&e, &[], &state).is_empty());
        state.autosave_ok = false;
        let d = published.diff(&e, &[], &state);
        assert_eq!(d.len(), 1);
        assert_eq!(
            parse(&d[0]),
            json!({"v":0,"type":"show","name":"Kerk","saved_at":"2026-10-18T09:00:05Z","autosave_ok":false})
        );
        assert!(
            published.diff(&e, &[], &state).is_empty(),
            "no change, no delta"
        );
        state.saved_at = Some("2026-10-18T09:00:10Z".into());
        assert_eq!(published.diff(&e, &[], &state).len(), 1);
    }

    #[test]
    fn shw_15_show_commands_parse() {
        use crate::shows::ShowRequest;
        let cases = [
            (r#"{"v":0,"cmd":"show.list"}"#, ShowRequest::List),
            (
                r#"{"v":0,"cmd":"show.new","name":"Kerst"}"#,
                ShowRequest::New {
                    name: "Kerst".into(),
                },
            ),
            (
                r#"{"v":0,"cmd":"show.open","name":"Kerk"}"#,
                ShowRequest::Open {
                    name: "Kerk".into(),
                },
            ),
            (
                r#"{"v":0,"cmd":"show.save_as","name":"Kerk 2"}"#,
                ShowRequest::SaveAs {
                    name: "Kerk 2".into(),
                },
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(parse_command(text), Ok(Command::Show(expected)), "{text}");
        }
        for bad in [
            r#"{"v":0,"cmd":"show.new"}"#,
            r#"{"v":0,"cmd":"show.open","name":3}"#,
            r#"{"v":0,"cmd":"show.save_as"}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn shw_15_list_answers_the_asking_client() {
        let dir = std::env::temp_dir().join(format!("cmdmx-shw-15-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let shows = crate::shows::Shows::new(dir.join("Shows"), dir.join("state.json"));
        let json = engine()
            .to_show(cmdmx_core::show::Meta {
                name: "Kerk".into(),
                created: "2026-10-18T09:00:00Z".into(),
                modified: "2026-10-18T09:00:00Z".into(),
                app_version: "0.1.0".into(),
            })
            .to_json();
        crate::shows::write_atomic(&shows.path("Kerk"), &json).unwrap();
        let v = parse(&answer_show_list(&shows));
        assert_eq!(v["v"], 0);
        assert_eq!(v["type"], "show.list");
        assert_eq!(v["shows"].as_array().unwrap().len(), 1);
        assert_eq!(v["shows"][0]["name"], "Kerk");
        assert!(v["shows"][0]["modified"].as_str().unwrap().ends_with('Z'));
    }

    // ---- PBK-03 --------------------------------------------------------------

    #[test]
    fn pbk_03_snapshot_has_the_page() {
        let v = parse(&snapshot(
            &engine(),
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        let slots: Vec<Value> = (1..=8)
            .map(|s| json!({"slot": s, "playback": if s == 1 { json!(1) } else { Value::Null }}))
            .collect();
        assert_eq!(v["page"], json!({"number": 1, "count": 10, "slots": slots}));
    }

    #[test]
    fn pbk_03_page_delta_only_after_a_change() {
        let mut e = engine();
        let mut published = Published::new(&e, &[], &show_state());
        assert!(published.diff(&e, &[], &show_state()).is_empty());
        e.set_page(2).unwrap();
        let d = published.diff(&e, &[], &show_state());
        assert_eq!(d.len(), 1);
        let v = parse(&d[0]);
        assert_eq!(v["v"], 0);
        assert_eq!(v["type"], "page");
        assert_eq!(v["number"], 2);
        assert_eq!(v["count"], 10);
        assert_eq!(v["slots"][0], json!({"slot": 1, "playback": null}));
        assert!(published.diff(&e, &[], &show_state()).is_empty());
    }

    #[test]
    fn pbk_03_page_set_parses() {
        assert_eq!(
            parse_command(r#"{"v":0,"cmd":"page.set","page":2}"#),
            Ok(Command::PageSet { page: 2 })
        );
        for bad in [
            r#"{"v":0,"cmd":"page.set"}"#,
            r#"{"v":0,"cmd":"page.set","page":"2"}"#,
            r#"{"v":0,"cmd":"page.set","page":-1}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
    }

    // ---- palettes (PAL-25..27, VS2-19) -----------------------------------------------------

    /// The test engine plus palette id 7 = number 2 "Full" (global Dimmer) in Beams cell 1 and
    /// id 8 = number 5 "Red" (global Colour, fits no fixture here) in Colours cell 1.
    fn engine_with_palettes() -> Engine {
        use cmdmx_core::colour::TargetColour;
        use cmdmx_core::palette::{GlobalValue, Palette, Window};
        let mut e = engine();
        let palette = |id, number, window, legend: &str, attribute: &str, value| Palette {
            id,
            number,
            window,
            handle: 1,
            legend: legend.into(),
            icon: None,
            global: BTreeMap::from([(attribute.to_string(), GlobalValue::Value(value))]),
            shared: BTreeMap::new(),
            fixtures: BTreeMap::new(),
        };
        e.add_palette(palette(
            8,
            5,
            Window::Colours,
            "Red",
            "Colour",
            cmdmx_core::value::Value::Colour(TargetColour {
                r: 1.0,
                g: 0.0,
                b: 0.0,
            }),
        ))
        .unwrap();
        e.add_palette(palette(
            7,
            2,
            Window::Beams,
            "Full",
            "Dimmer",
            cmdmx_core::value::Value::Level(1.0),
        ))
        .unwrap();
        e
    }

    #[test]
    fn pal_25_snapshot_lists_palettes_by_number() {
        let mut e = engine_with_palettes();
        e.set_palette_legend(2, "Full", Some("beam.zoom")).unwrap();
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert_eq!(
            v["palettes"],
            json!([
                {"id":7,"number":2,"window":"Beams","handle":1,"legend":"Full","icon":"beam.zoom",
                 "auto_icon":{"value":{"attribute":"Dimmer","value":1.0}},"kind":"G","groups":["I"]},
                {"id":8,"number":5,"window":"Colours","handle":1,"legend":"Red","icon":null,
                 "auto_icon":{"colours":[{"r":1.0,"g":0.0,"b":0.0}]},"kind":"G","groups":["C"]}
            ])
        );
    }

    #[test]
    fn pal_25_palettes_delta_after_a_change_only() {
        let mut e = engine_with_palettes();
        let mut published = Published::new(&e, &[], &show_state());
        e.tick(0.0);
        assert!(published.diff(&e, &[], &show_state()).is_empty());
        e.set_palette_legend(5, "Warm", None).unwrap();
        e.tick(0.025);
        let d = published.diff(&e, &[], &show_state());
        assert_eq!(d.len(), 1, "{d:?}");
        let v = parse(&d[0]);
        assert_eq!(v["v"], 0);
        assert_eq!(v["type"], "palettes");
        assert_eq!(v["palettes"].as_array().unwrap().len(), 2);
        assert_eq!(v["palettes"][1]["legend"], "Warm");
        e.tick(0.05);
        assert!(
            published.diff(&e, &[], &show_state()).is_empty(),
            "no change, no delta"
        );
        e.move_palette(
            5,
            cmdmx_core::palette::Cell::new(cmdmx_core::palette::Window::Positions, 3),
        )
        .unwrap();
        e.tick(0.075);
        let d = published.diff(&e, &[], &show_state());
        assert_eq!(d.len(), 1);
        assert_eq!(parse(&d[0])["palettes"][1]["window"], "Positions");
    }

    #[test]
    fn pal_25_palette_settings_in_the_snapshot_and_a_delta_on_change() {
        let mut e = engine_with_palettes();
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert_eq!(
            v["palette_settings"],
            json!({"time":0.0,"overlap":100.0,"record_by":"Mixed"})
        );
        let mut published = Published::new(&e, &[], &show_state());
        e.set_palette_master(Some(2.0), Some(50.0)).unwrap();
        e.set_record_by(cmdmx_core::palette::RecordBy::Channel);
        let d = published.diff(&e, &[], &show_state());
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(
            parse(&d[0]),
            json!({"v":0,"type":"palette_settings","time":2.0,"overlap":50.0,"record_by":"Channel"})
        );
        assert!(published.diff(&e, &[], &show_state()).is_empty());
    }

    #[test]
    fn pal_26_programmer_state_carries_palette_use() {
        let mut e = engine_with_palettes();
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert_eq!(
            v["programmer"]["palettes"],
            json!({"in_use":[],"relevant":[7,8]}),
            "nothing selected: every palette is relevant (quick palette)"
        );
        e.select_fixture(1).unwrap();
        e.apply_palette(2, cmdmx_core::palette::Timing::default())
            .unwrap();
        let p = &parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ))["programmer"];
        assert_eq!(p["palettes"], json!({"in_use":[7],"relevant":[7]}));
        assert_eq!(
            p["wheels"],
            json!([{"attribute":"Dimmer","value":1.0,"touched":true,"sixteen_bit":false,"palette":7}])
        );
        assert_eq!(
            p["contents"],
            json!([{"fixture":1,"values":{"Dimmer":1.0},"palettes":{"Dimmer":7},"fading":{}}])
        );
    }

    #[test]
    fn pal_27_palette_commands_parse() {
        use cmdmx_core::palette::{Cell, Record, RecordAs, RecordBy, RecordMode, Timing, Window};
        let ok = |s: &str| parse_command(s).unwrap();
        assert_eq!(
            ok(r#"{"v":0,"cmd":"palette.record","window":"Colours","handle":4}"#),
            Command::PaletteRecord(Record::at(Cell::new(Window::Colours, 4)))
        );
        assert_eq!(
            ok(
                r#"{"v":0,"cmd":"palette.record","window":"Beams","handle":2,"number":12,
                  "mask":["I","B"],"record_by":"Channel","record_as":"Shared","mode":"QuickMerge"}"#
            ),
            Command::PaletteRecord(Record {
                number: Some(12),
                mask: Some(vec![AttributeGroup::I, AttributeGroup::B]),
                record_by: Some(RecordBy::Channel),
                record_as: RecordAs::Shared,
                mode: Some(RecordMode::QuickMerge),
                ..Record::at(Cell::new(Window::Beams, 2))
            })
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"palette.apply","number":3}"#),
            Command::PaletteApply {
                number: 3,
                timing: Timing::default()
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"palette.apply","number":3,"time":2.0,"overlap":50}"#),
            Command::PaletteApply {
                number: 3,
                timing: Timing {
                    time: Some(2.0),
                    overlap: Some(50.0)
                }
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"palette.include","number":3}"#),
            Command::PaletteInclude { number: 3 }
        );
        let to = Cell::new(Window::Colours, 5);
        assert_eq!(
            ok(r#"{"v":0,"cmd":"palette.copy","number":3,"to":{"window":"Colours","handle":5}}"#),
            Command::PaletteCopy { number: 3, to }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"palette.move","number":3,"to":{"window":"Colours","handle":5}}"#),
            Command::PaletteMove { number: 3, to }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"palette.renumber","number":3,"to":30}"#),
            Command::PaletteRenumber { number: 3, to: 30 }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"palette.delete","number":3}"#),
            Command::PaletteDelete { number: 3 }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"palette.legend","number":3,"legend":"Warm","icon":"colour.drop"}"#),
            Command::PaletteLegend {
                number: 3,
                legend: "Warm".into(),
                icon: Some("colour.drop".into())
            }
        );
        for automatic in [
            r#"{"v":0,"cmd":"palette.legend","number":3,"legend":"Warm","icon":null}"#,
            r#"{"v":0,"cmd":"palette.legend","number":3,"legend":"Warm"}"#,
        ] {
            assert_eq!(
                ok(automatic),
                Command::PaletteLegend {
                    number: 3,
                    legend: "Warm".into(),
                    icon: None
                }
            );
        }
        assert_eq!(
            ok(r#"{"v":0,"cmd":"settings.palette","time":2.0}"#),
            Command::SettingsPalette {
                time: Some(2.0),
                overlap: None,
                record_by: None
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"settings.palette","overlap":50,"record_by":"Channel"}"#),
            Command::SettingsPalette {
                time: None,
                overlap: Some(50.0),
                record_by: Some(RecordBy::Channel)
            }
        );
    }

    #[test]
    fn pal_27_bad_palette_commands_are_errors() {
        for bad in [
            r#"{"v":0,"cmd":"palette.record","window":"Colours"}"#,
            r#"{"v":0,"cmd":"palette.record","window":"Gobos","handle":1}"#,
            r#"{"v":0,"cmd":"palette.record","window":"Colours","handle":"1"}"#,
            r#"{"v":0,"cmd":"palette.record","window":"Colours","handle":1,"mode":"Overwrite"}"#,
            r#"{"v":0,"cmd":"palette.apply"}"#,
            r#"{"v":0,"cmd":"palette.apply","number":3,"time":"2"}"#,
            r#"{"v":0,"cmd":"palette.copy","number":3}"#,
            r#"{"v":0,"cmd":"palette.move","number":3,"to":{"window":"Colours"}}"#,
            r#"{"v":0,"cmd":"palette.renumber","number":3}"#,
            r#"{"v":0,"cmd":"palette.delete","number":-1}"#,
            r#"{"v":0,"cmd":"palette.legend","number":3}"#,
            r#"{"v":0,"cmd":"settings.palette","record_by":"Group"}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
    }

    // ---- playbacks (PBK-35, 36, VS2-22) -------------------------------------------------------

    /// PBK-35: the test engine's playback 1 (1.1, Dimmer 1.0 on fixture 1), untouched.
    fn playback_entry() -> Value {
        json!({"id":1,"page":1,"slot":1,"legend":"","icon":null,
               "auto_icon":{"value":{"attribute":"Dimmer","value":1.0}},"halo":null,
               "level":0.0,"state":"off","flash":false,"latched":false,"fired":null,
               "groups":["I"],
               "options":{"priority":"Normal","fader_mode":0,"kill_point":"FadeOutComplete",
                          "release_mask":null,"release_time":null,"paging":"Unlocked","group":null},
               "times":{"delay":0.0,"fade_in":0.0,"fade_out":0.0,"overlap":100.0},
               "kind":"Cues",
               "cues":[{"number":1.0,"legend":"","delay":0.0,"fade_in":0.0,"fade_out":0.0,
                        "overlap":100.0,"link":{"type":"Wait"},"track":"Track",
                        "group_times":{},"groups":["I"]}],
               "list":{"tracking":true,"fader":"KillWithOff","fire_first_cue":true,
                       "loop":"StopOnFinal"},
               "chase":{"bpm":120.0,"crossfade":0.0,"direction":"Forward","loop":"Loop",
                        "speed_source":"Free"},
               "master":null,
               "buttons":["Swop","Flash"],
               "cue":null,"bpm":null})
    }

    #[test]
    fn pbk_35_snapshot_playback_entries() {
        let mut e = engine();
        e.set_playback_legend(
            1,
            "Walk-in",
            Some("service.walk-in"),
            Some(cmdmx_core::colour::TargetColour {
                r: 1.0,
                g: 0.5,
                b: 0.0,
            }),
        )
        .unwrap();
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        let mut want = playback_entry();
        want["legend"] = json!("Walk-in");
        want["icon"] = json!("service.walk-in");
        want["halo"] = json!({"r":1.0,"g":0.5,"b":0.0});
        assert_eq!(v["playbacks"], json!([want]));
    }

    #[test]
    fn pbk_35_snapshot_pages_dbo_and_playback_settings() {
        let mut e = engine();
        e.set_page_legend(2, "Service").unwrap();
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        let pages: Vec<Value> = (1..=10)
            .map(|n| json!({"number": n, "legend": if n == 2 { "Service" } else { "" }}))
            .collect();
        assert_eq!(v["pages"], json!(pages));
        assert_eq!(v["dbo"], json!(false));
        assert_eq!(
            v["playback_settings"],
            json!({"record_by":"Channel","delay":0.0,"fade_in":0.0,"fade_out":0.0,
                   "overlap":100.0,"release_mask":[],"release_time":2.0,
                   "auto_connect":true,"cue_list_fader":"KillWithOff","update_mode":"Forwards",
                   "handle_buttons":"BestPractice"})
        );
    }

    #[test]
    fn pbk_35_live_changes_are_small_playback_deltas() {
        let mut e = engine();
        let mut published = Published::new(&e, &[], &show_state());
        e.set_flash(1, true, 0.0).unwrap();
        e.tick(0.0);
        let d = published.diff(&e, &[], &show_state());
        assert_eq!(d.len(), 1, "{d:?}");
        let mut v = parse(&d[0]);
        assert!(
            v["fired"].is_u64(),
            "a fired playback has a fire order: {v}"
        );
        v.as_object_mut().unwrap().remove("fired");
        assert_eq!(
            v,
            json!({"v":0,"type":"playback","id":1,"level":0.0,"state":"active",
                   "flash":true,"latched":false,"cue":null,"bpm":null})
        );
        e.set_flash(1, false, 0.025).unwrap();
        e.latch_flash(1, 0.025).unwrap();
        e.tick(0.025);
        let d = published.diff(&e, &[], &show_state());
        let mut v = parse(&d[0]);
        v.as_object_mut().unwrap().remove("fired");
        assert_eq!(
            v,
            json!({"v":0,"type":"playback","id":1,"level":0.0,"state":"active",
                   "flash":true,"latched":true,"cue":null,"bpm":null}),
            "flash means held or latched"
        );
    }

    #[test]
    fn pbk_35_fired_orders_the_active_playbacks() {
        let mut e = engine();
        e.add_playback(Playback::new(
            2,
            1,
            2,
            BTreeMap::from([(1, BTreeMap::from([("Dimmer".to_string(), 0.5.into())]))]),
        ));
        e.set_fader(2, 1.0, 0.0).unwrap();
        e.set_fader(1, 1.0, 0.0).unwrap();
        e.tick(0.0);
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        let fired = |i: usize| v["playbacks"][i]["fired"].as_u64().unwrap();
        assert!(fired(1) < fired(0), "playback 2 fired first");
        e.set_fader(1, 0.0, 0.025).unwrap();
        e.tick(0.025);
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert!(v["playbacks"][0]["fired"].is_null(), "off again");
    }

    #[test]
    fn pbk_35_releasing_is_a_state() {
        let mut e = engine();
        e.add_profile(
            "mover",
            Profile::from_attributes(vec![AttributeDef::new(
                "Pan",
                AttributeGroup::P,
                AttributeKind::Ltp,
                1,
            )]),
        )
        .unwrap();
        e.patch(Fixture {
            id: 2,
            number: 0,
            name: String::new(),
            handle: None,
            profile: "mover".into(),
            mode: None,
            universe: 1,
            address: 5,
        })
        .unwrap();
        let mut pb = Playback::new(
            2,
            1,
            2,
            BTreeMap::from([(2, BTreeMap::from([("Pan".to_string(), 1.0.into())]))]),
        );
        pb.release_mask = Some(vec![AttributeGroup::P]);
        pb.release_time = Some(4.0);
        e.add_playback(pb);
        e.set_fader(2, 1.0, 0.0).unwrap();
        e.tick(0.0);
        let mut published = Published::new(&e, &[], &show_state());
        e.set_fader(2, 0.0, 0.025).unwrap();
        e.tick(0.025);
        let d = published.diff(&e, &[], &show_state());
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(parse(&d[0])["state"], "releasing");
    }

    #[test]
    fn pbk_35_stored_changes_send_the_whole_list() {
        let mut e = engine();
        let mut published = Published::new(&e, &[], &show_state());
        e.set_fader(1, 0.5, 0.0).unwrap();
        e.tick(0.0);
        let d = published.diff(&e, &[], &show_state());
        assert!(
            d.iter().all(|m| parse(m)["type"] == "playback"),
            "a fader move is not a stored change: {d:?}"
        );
        e.set_playback_legend(1, "Front", None, None).unwrap();
        e.copy_playback(1, 2, 3).unwrap();
        e.tick(0.025);
        let d = published.diff(&e, &[], &show_state());
        assert_eq!(
            d.iter().filter(|m| parse(m)["type"] == "playbacks").count(),
            1,
            "one whole-list delta for both changes: {d:?}"
        );
        let list = d
            .iter()
            .map(|m| parse(m))
            .find(|v| v["type"] == "playbacks")
            .unwrap();
        let entries = list["playbacks"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["legend"], "Front");
        assert_eq!(
            entries[0]["level"], 0.5,
            "entries carry the live values too"
        );
        assert_eq!(entries[1]["page"], 2);
        e.delete_playback(2).unwrap();
        e.tick(0.05);
        let d = published.diff(&e, &[], &show_state());
        let list = d
            .iter()
            .map(|m| parse(m))
            .find(|v| v["type"] == "playbacks")
            .unwrap();
        assert_eq!(
            list["playbacks"].as_array().unwrap().len(),
            1,
            "a delete is in the list"
        );
        e.tick(0.075);
        assert!(published.diff(&e, &[], &show_state()).is_empty());
    }

    #[test]
    fn pbk_35_pages_dbo_and_settings_deltas() {
        let mut e = engine();
        let mut published = Published::new(&e, &[], &show_state());
        e.set_page_legend(3, "Video").unwrap();
        e.set_dbo(true);
        e.set_playback_settings(&cmdmx_core::engine::PlaybackSettingsChange {
            release_time: Some(1.0),
            ..Default::default()
        })
        .unwrap();
        e.tick(0.0);
        let d: Vec<Value> = published
            .diff(&e, &[], &show_state())
            .iter()
            .map(|m| parse(m))
            .collect();
        assert_eq!(d.len(), 3, "{d:?}");
        let of = |t: &str| d.iter().find(|v| v["type"] == t).unwrap().clone();
        assert_eq!(
            of("pages")["pages"][2],
            json!({"number":3,"legend":"Video"})
        );
        assert_eq!(of("dbo"), json!({"v":0,"type":"dbo","on":true}));
        assert_eq!(of("playback_settings")["release_time"], 1.0);
        e.tick(0.025);
        assert!(published.diff(&e, &[], &show_state()).is_empty());
    }

    #[test]
    fn pbk_36_playback_commands_parse() {
        use cmdmx_core::colour::TargetColour;
        use cmdmx_core::engine::PlaybackSettingsChange;
        use cmdmx_core::palette::{RecordBy, RecordMode};
        use cmdmx_core::playback::{
            FaderMode, KillPoint, Paging, PlaybackChange, PlaybackRecord, Priority,
        };
        let ok = |s: &str| parse_command(s).unwrap();
        assert_eq!(
            ok(r#"{"v":0,"cmd":"playback.record","page":1,"slot":2}"#),
            Command::PlaybackRecord(PlaybackRecord::at(1, 2))
        );
        assert_eq!(
            ok(
                r#"{"v":0,"cmd":"playback.record","page":1,"slot":2,"mask":["I","C"],
                   "record_by":"Mixed","mode":"Update"}"#
            ),
            Command::PlaybackRecord(PlaybackRecord {
                mask: Some(vec![AttributeGroup::I, AttributeGroup::C]),
                record_by: Some(RecordBy::Mixed),
                mode: Some(RecordMode::QuickMerge),
                ..PlaybackRecord::at(1, 2)
            })
        );
        for (mode, want) in [
            ("Replace", RecordMode::Replace),
            ("Merge", RecordMode::Merge),
        ] {
            let text =
                format!(r#"{{"v":0,"cmd":"playback.record","page":1,"slot":2,"mode":"{mode}"}}"#);
            assert_eq!(
                ok(&text),
                Command::PlaybackRecord(PlaybackRecord {
                    mode: Some(want),
                    ..PlaybackRecord::at(1, 2)
                })
            );
        }
        assert_eq!(
            ok(r#"{"v":0,"cmd":"playback.flash","id":3,"on":true}"#),
            Command::PlaybackFlash { id: 3, on: true }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"playback.swop","id":3,"on":false}"#),
            Command::PlaybackSwop { id: 3, on: false }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"playback.latch","id":3}"#),
            Command::PlaybackLatch { id: 3 }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"playback.release","id":3}"#),
            Command::PlaybackRelease { id: 3 }
        );
        assert_eq!(
            ok(
                r#"{"v":0,"cmd":"playback.options","id":3,"priority":"High","fader_mode":1,
                   "kill_point":"FaderAtZero","release_mask":["I"],"release_time":0.5,
                   "paging":"Locked","group":4}"#
            ),
            Command::PlaybackChange {
                id: 3,
                change: PlaybackChange {
                    priority: Some(Priority::High),
                    fader_mode: Some(FaderMode::Mode1),
                    kill_point: Some(KillPoint::FaderAtZero),
                    release_mask: Some(Some(vec![AttributeGroup::I])),
                    release_time: Some(Some(0.5)),
                    paging: Some(Paging::Locked),
                    group: Some(Some(4)),
                    ..PlaybackChange::default()
                }
            }
        );
        assert_eq!(
            ok(
                r#"{"v":0,"cmd":"playback.options","id":3,"release_mask":null,"release_time":null,"group":null}"#
            ),
            Command::PlaybackChange {
                id: 3,
                change: PlaybackChange {
                    release_mask: Some(None),
                    release_time: Some(None),
                    group: Some(None),
                    ..PlaybackChange::default()
                }
            },
            "null = back to the global setting / no group"
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"playback.options","id":3}"#),
            Command::PlaybackChange {
                id: 3,
                change: PlaybackChange::default()
            },
            "absent = unchanged"
        );
        assert_eq!(
            ok(
                r#"{"v":0,"cmd":"playback.times","id":3,"delay":1.0,"fade_in":3.0,"fade_out":4.0,"overlap":50}"#
            ),
            Command::PlaybackChange {
                id: 3,
                change: PlaybackChange {
                    delay: Some(1.0),
                    fade_in: Some(3.0),
                    fade_out: Some(4.0),
                    overlap: Some(50.0),
                    ..PlaybackChange::default()
                }
            }
        );
        assert_eq!(
            ok(
                r#"{"v":0,"cmd":"playback.legend","id":3,"legend":"Walk-in","icon":"service.walk-in",
                   "halo":{"r":1.0,"g":0.5,"b":0.0}}"#
            ),
            Command::PlaybackLegend {
                id: 3,
                legend: "Walk-in".into(),
                icon: Some("service.walk-in".into()),
                halo: Some(TargetColour {
                    r: 1.0,
                    g: 0.5,
                    b: 0.0
                }),
            }
        );
        for automatic in [
            r#"{"v":0,"cmd":"playback.legend","id":3,"legend":"x","icon":null,"halo":null}"#,
            r#"{"v":0,"cmd":"playback.legend","id":3,"legend":"x"}"#,
        ] {
            assert_eq!(
                ok(automatic),
                Command::PlaybackLegend {
                    id: 3,
                    legend: "x".into(),
                    icon: None,
                    halo: None
                }
            );
        }
        assert_eq!(
            ok(r#"{"v":0,"cmd":"playback.copy","id":3,"to":{"page":2,"slot":4}}"#),
            Command::PlaybackCopy {
                id: 3,
                page: 2,
                slot: 4
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"playback.move","id":3,"to":{"page":2,"slot":4}}"#),
            Command::PlaybackMove {
                id: 3,
                page: 2,
                slot: 4
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"playback.delete","id":3}"#),
            Command::PlaybackDelete { id: 3 }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"page.legend","page":2,"legend":"Service"}"#),
            Command::PageLegend {
                page: 2,
                legend: "Service".into()
            }
        );
        assert_eq!(
            ok(r#"{"v":0,"cmd":"dbo.set","on":true}"#),
            Command::DboSet { on: true }
        );
        assert_eq!(
            ok(
                r#"{"v":0,"cmd":"settings.playback","record_by":"Mixed","fade_in":2.0,
                   "release_mask":["I","P"],"release_time":1.0}"#
            ),
            Command::SettingsPlayback(PlaybackSettingsChange {
                record_by: Some(RecordBy::Mixed),
                fade_in: Some(2.0),
                release_mask: Some(vec![AttributeGroup::I, AttributeGroup::P]),
                release_time: Some(1.0),
                ..PlaybackSettingsChange::default()
            })
        );
    }

    #[test]
    fn pbk_36_bad_playback_commands_are_errors() {
        for bad in [
            r#"{"v":0,"cmd":"playback.record","page":1}"#,
            r#"{"v":0,"cmd":"playback.record","page":1,"slot":2,"mode":"QuickMerge"}"#,
            r#"{"v":0,"cmd":"playback.record","page":1,"slot":2,"mask":["Q"]}"#,
            r#"{"v":0,"cmd":"playback.flash","id":3}"#,
            r#"{"v":0,"cmd":"playback.swop","id":3,"on":"yes"}"#,
            r#"{"v":0,"cmd":"playback.latch"}"#,
            r#"{"v":0,"cmd":"playback.release","id":-1}"#,
            r#"{"v":0,"cmd":"playback.options","id":3,"fader_mode":3}"#,
            r#"{"v":0,"cmd":"playback.options","id":3,"paging":"Sticky"}"#,
            r#"{"v":0,"cmd":"playback.options","id":3,"priority":"Highest"}"#,
            r#"{"v":0,"cmd":"playback.times","id":3,"fade_in":"3"}"#,
            r#"{"v":0,"cmd":"playback.legend","id":3}"#,
            r#"{"v":0,"cmd":"playback.legend","id":3,"legend":"x","halo":"red"}"#,
            r#"{"v":0,"cmd":"playback.copy","id":3,"to":{"page":2}}"#,
            r#"{"v":0,"cmd":"playback.move","id":3}"#,
            r#"{"v":0,"cmd":"playback.delete"}"#,
            r#"{"v":0,"cmd":"page.legend","page":2}"#,
            r#"{"v":0,"cmd":"dbo.set"}"#,
            r#"{"v":0,"cmd":"settings.playback","record_by":"Fixture"}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
    }

    // ---- VS3-05: cue lists over IPC (CUE-40, 41, 54; IPC-24..28) ------------------------------

    use cmdmx_core::playback::{Cue, CueNumber, Kind, MasterKind};

    fn q(n: f64) -> CueNumber {
        CueNumber::new(n).unwrap()
    }

    fn dimmer(fixture: u32, level: f32) -> cmdmx_core::playback::Values {
        BTreeMap::from([(
            fixture,
            BTreeMap::from([("Dimmer".to_string(), level.into())]),
        )])
    }

    /// `engine()` plus fixture 2 on channel 2 and list 2 on 1.2: Q1 fixture 1 at full, Q2
    /// "Welcome" fixture 2 at 50 % with a 2 s fade in.
    fn list_engine() -> Engine {
        let mut e = engine();
        e.patch(Fixture {
            id: 2,
            number: 0,
            name: String::new(),
            handle: None,
            profile: "dimmer".into(),
            mode: None,
            universe: 1,
            address: 2,
        })
        .unwrap();
        let q1 = Cue::new(dimmer(1, 1.0));
        let q2 = Cue {
            number: q(2.0),
            legend: "Welcome".into(),
            fade_in: 2.0,
            ..Cue::new(dimmer(2, 0.5))
        };
        e.add_playback(Playback {
            cues: vec![q1, q2],
            ..Playback::new(2, 1, 2, BTreeMap::new())
        });
        e
    }

    fn entry(v: &Value, id: u64) -> Value {
        v["playbacks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == json!(id))
            .cloned()
            .unwrap()
    }

    #[test]
    fn cue_40_list_entry_has_cues_buttons_and_live_cue() {
        let e = list_engine();
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        let p = entry(&v, 2);
        assert_eq!(p["kind"], json!("Cues"));
        assert_eq!(p["cues"].as_array().unwrap().len(), 2);
        assert_eq!(p["cues"][1]["number"], json!(2.0));
        assert_eq!(p["cues"][1]["legend"], json!("Welcome"));
        assert_eq!(p["cues"][1]["fade_in"], json!(2.0));
        assert!(
            p["cues"][1].get("values").is_none(),
            "cues come without values"
        );
        assert_eq!(p["buttons"], json!(["Go", "Flash"]));
        assert_eq!(
            p["cue"],
            json!({"current":null,"next":1.0,"progress":0.0,"paused":false})
        );
        assert_eq!(p["bpm"], json!(null));
        assert_eq!(
            v["connected"],
            json!(2),
            "the first list when none is connected"
        );
    }

    #[test]
    fn cue_40_snapshot_without_a_list_has_no_connected_list() {
        let v = parse(&snapshot(
            &engine(),
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert_eq!(v["connected"], json!(null));
    }

    #[test]
    fn cue_40_a_running_transition_sends_small_deltas_with_its_progress() {
        let mut e = list_engine();
        let mut published = Published::new(&e, &[], &show_state());
        e.cuelist(Some(2), cmdmx_core::playback::ListAction::Go, 0.0)
            .unwrap();
        e.tick(0.0);
        let _ = published.diff(&e, &[], &show_state());
        e.cuelist(Some(2), cmdmx_core::playback::ListAction::Go, 0.025)
            .unwrap();
        e.tick(0.025);
        e.tick(0.5);
        let d = published.diff(&e, &[], &show_state());
        let deltas: Vec<Value> = d.iter().map(|m| parse(m)).collect();
        let mine: Vec<&Value> = deltas
            .iter()
            .filter(|m| m["type"] == json!("playback") && m["id"] == json!(2))
            .collect();
        assert_eq!(mine.len(), 1, "one per tick: {deltas:?}");
        let cue = &mine[0]["cue"];
        assert_eq!(cue["current"], json!(2.0));
        assert_eq!(cue["next"], json!(null));
        let progress = cue["progress"].as_f64().unwrap();
        assert!((0.2..0.3).contains(&progress), "{progress}");
        assert!(
            !deltas.iter().any(|m| m["type"] == json!("playbacks")),
            "live changes never send the whole list"
        );
    }

    #[test]
    fn cue_40_connected_delta_on_change() {
        let mut e = list_engine();
        e.add_playback(Playback {
            kind: Kind::Chase,
            cues: vec![Cue::new(dimmer(1, 1.0)), Cue::new(dimmer(2, 1.0))],
            ..Playback::new(3, 1, 3, BTreeMap::new())
        });
        let mut published = Published::new(&e, &[], &show_state());
        e.connect_list(3).unwrap();
        let d = published.diff(&e, &[], &show_state());
        assert!(
            d.iter()
                .any(|m| parse(m) == json!({"v":0,"type":"connected","id":3})),
            "{d:?}"
        );
        assert!(published.diff(&e, &[], &show_state()).is_empty());
    }

    #[test]
    fn cue_40_chases_and_bpm_masters_carry_their_tempo() {
        let mut e = list_engine();
        e.add_playback(Playback {
            kind: Kind::Chase,
            cues: vec![Cue::new(dimmer(1, 1.0)), Cue::new(dimmer(2, 1.0))],
            ..Playback::new(3, 1, 3, BTreeMap::new())
        });
        let master = e.record_master(1, 4, MasterKind::Bpm, 1).unwrap();
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        let chase = entry(&v, 3);
        assert_eq!(chase["kind"], json!("Chase"));
        assert_eq!(chase["bpm"], json!(120.0));
        assert_eq!(chase["buttons"], json!(["Tap", "Flash"]));
        assert!(chase["cue"].is_object());
        let m = entry(&v, u64::from(master));
        assert_eq!(m["kind"], json!("BpmMaster"));
        assert_eq!(
            m["master"],
            json!({"number":1,"bpm":120.0,"multiplier":1.0,"fader":"None","fader_max":240.0})
        );
        assert_eq!(m["bpm"], json!(120.0));
        assert_eq!(m["buttons"], json!(["None", "Tap"]));
        assert_eq!(m["cue"], json!(null));
    }

    #[test]
    fn cue_40_a_cue_edit_sends_the_whole_list() {
        let mut e = list_engine();
        let mut published = Published::new(&e, &[], &show_state());
        e.change_cue(
            2,
            q(2.0),
            &cmdmx_core::playback::CueChange {
                legend: Some("Worship".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let d = published.diff(&e, &[], &show_state());
        let whole = d
            .iter()
            .map(|m| parse(m))
            .find(|m| m["type"] == json!("playbacks"))
            .expect("a playbacks delta");
        assert_eq!(entry(&whole, 2)["cues"][1]["legend"], json!("Worship"));
    }

    #[test]
    fn cue_41_commands_parse() {
        for good in [
            r#"{"v":0,"cmd":"cuelist.go"}"#,
            r#"{"v":0,"cmd":"cuelist.go","id":2}"#,
            r#"{"v":0,"cmd":"cuelist.back"}"#,
            r#"{"v":0,"cmd":"cuelist.stop","id":2}"#,
            r#"{"v":0,"cmd":"cuelist.goto","cue":2.5}"#,
            r#"{"v":0,"cmd":"cuelist.next","id":2,"cue":3}"#,
            r#"{"v":0,"cmd":"cuelist.connect","id":2}"#,
            r#"{"v":0,"cmd":"playback.release"}"#,
            r#"{"v":0,"cmd":"playback.button","id":2,"button":"upper","on":true}"#,
            r#"{"v":0,"cmd":"playback.tap","id":3}"#,
            r#"{"v":0,"cmd":"playback.bpm","id":3,"bpm":128}"#,
            r#"{"v":0,"cmd":"playback.record","page":1,"slot":2,"mode":"Append"}"#,
            r#"{"v":0,"cmd":"playback.record","page":1,"slot":2,"mode":"Insert"}"#,
            r#"{"v":0,"cmd":"playback.record","page":1,"slot":2,"cue":4}"#,
            r#"{"v":0,"cmd":"playback.record","page":1,"slot":2,"cue":2,"mode":"Update",
                "update_mode":"Backwards"}"#,
            r#"{"v":0,"cmd":"playback.options","id":2,"kind":"Chase"}"#,
            r#"{"v":0,"cmd":"playback.options","id":2,"list":{"tracking":false}}"#,
            r#"{"v":0,"cmd":"playback.options","id":3,"chase":{"bpm":90,"direction":"Bounce",
                "speed_source":{"Bpm":1}}}"#,
            r#"{"v":0,"cmd":"playback.options","id":4,"master":{"multiplier":2}}"#,
            r#"{"v":0,"cmd":"cue.times","id":2,"cue":2,"fade_in":3,
                "link":{"type":"After","offset":1},"group_times":{"P":{"delay":1,"fade":5}}}"#,
            r#"{"v":0,"cmd":"cue.legend","id":2,"cue":2,"legend":"Welcome"}"#,
            r#"{"v":0,"cmd":"cue.track","id":2,"cue":2,"track":"CueOnly"}"#,
            r#"{"v":0,"cmd":"cue.delete","id":2,"cue":2}"#,
            r#"{"v":0,"cmd":"cue.renumber","id":2,"cue":2,"to":2.5}"#,
            r#"{"v":0,"cmd":"cue.renumber_all","id":2}"#,
            r#"{"v":0,"cmd":"cue.copy","id":2,"cue":2,"to":7}"#,
            r#"{"v":0,"cmd":"cue.view","id":2,"cue":2}"#,
            r#"{"v":0,"cmd":"cue.remove","id":2,"cue":2,"fixture":201}"#,
            r#"{"v":0,"cmd":"cue.remove","id":2,"cue":2,"fixture":201,"attribute":"Dimmer"}"#,
            r#"{"v":0,"cmd":"master.record","page":1,"slot":4,"kind":"Bpm"}"#,
            r#"{"v":0,"cmd":"master.record","page":1,"slot":4,"kind":"Rate","number":2}"#,
            r#"{"v":0,"cmd":"settings.playback","auto_connect":false,"cue_list_fader":"KillAtZero",
                "update_mode":"Both","handle_buttons":{"Custom":{"CueList":["Go","Back"]}}}"#,
        ] {
            assert!(
                parse_command(good).is_ok(),
                "{good}: {:?}",
                parse_command(good)
            );
        }
    }

    #[test]
    fn cue_41_bad_commands_are_errors() {
        for bad in [
            r#"{"v":0,"cmd":"cuelist.go","id":"two"}"#,
            r#"{"v":0,"cmd":"cuelist.goto","id":2}"#,
            r#"{"v":0,"cmd":"cuelist.next","cue":"x"}"#,
            r#"{"v":0,"cmd":"cuelist.goto","cue":2.0001}"#,
            r#"{"v":0,"cmd":"cuelist.connect"}"#,
            r#"{"v":0,"cmd":"playback.button","id":2,"button":"middle","on":true}"#,
            r#"{"v":0,"cmd":"playback.button","id":2,"button":"upper"}"#,
            r#"{"v":0,"cmd":"playback.tap"}"#,
            r#"{"v":0,"cmd":"playback.bpm","id":3}"#,
            r#"{"v":0,"cmd":"playback.record","page":1,"slot":2,"mode":"Append","cue":3}"#,
            r#"{"v":0,"cmd":"playback.record","page":1,"slot":2,"update_mode":"Sideways"}"#,
            r#"{"v":0,"cmd":"playback.options","id":2,"kind":"Timeline"}"#,
            r#"{"v":0,"cmd":"playback.options","id":3,"chase":{"direction":"Sideways"}}"#,
            r#"{"v":0,"cmd":"playback.options","id":3,"list":"Loop"}"#,
            r#"{"v":0,"cmd":"cue.times","id":2,"fade_in":3}"#,
            r#"{"v":0,"cmd":"cue.times","id":2,"cue":2,"link":{"type":"Later"}}"#,
            r#"{"v":0,"cmd":"cue.legend","id":2,"cue":2}"#,
            r#"{"v":0,"cmd":"cue.track","id":2,"cue":2,"track":"Hold"}"#,
            r#"{"v":0,"cmd":"cue.delete","id":2}"#,
            r#"{"v":0,"cmd":"cue.renumber","id":2,"cue":2}"#,
            r#"{"v":0,"cmd":"cue.renumber_all"}"#,
            r#"{"v":0,"cmd":"cue.copy","id":2,"cue":2}"#,
            r#"{"v":0,"cmd":"cue.view","id":2}"#,
            r#"{"v":0,"cmd":"cue.remove","id":2,"cue":2}"#,
            r#"{"v":0,"cmd":"master.record","page":1,"slot":4,"kind":"Size"}"#,
            r#"{"v":0,"cmd":"master.record","page":1,"kind":"Bpm"}"#,
            r#"{"v":0,"cmd":"settings.playback","handle_buttons":"Avolites"}"#,
            r#"{"v":0,"cmd":"settings.playback","update_mode":"Sideways"}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn cue_54_cue_view_message() {
        let e = list_engine();
        let v = parse(&cue_view(&e, 2, q(2.0)).unwrap());
        assert_eq!(
            v,
            json!({"v":0,"type":"cue.view","playback":2,"cue":2.0,"fixtures":[
                {"fixture":1,"values":{"Dimmer":{"value":1.0,"from":1.0}}},
                {"fixture":2,"values":{"Dimmer":{"value":0.5,"from":2.0}}}]})
        );
        assert!(cue_view(&e, 2, q(9.0)).is_err());
        assert!(cue_view(&e, 9, q(1.0)).is_err());
    }

    #[test]
    fn ipc_25_stamp_sets_the_receive_time_of_taps_and_buttons_only() {
        let tap = parse_command(r#"{"v":0,"cmd":"playback.tap","id":3}"#).unwrap();
        assert_ne!(stamp(tap.clone(), 1.5), tap, "a tap carries its time");
        let button =
            parse_command(r#"{"v":0,"cmd":"playback.button","id":3,"button":"upper","on":true}"#)
                .unwrap();
        assert_ne!(stamp(button.clone(), 1.5), button);
        let go = parse_command(r#"{"v":0,"cmd":"cuelist.go"}"#).unwrap();
        assert_eq!(stamp(go.clone(), 1.5), go);
    }

    // ---- VS3-07: group masters over IPC (GRM-15, IPC-29) --------------------------------------

    /// `engine()` with fixture 1 in group 6 and a group master for it on 1.8.
    fn group_master_engine() -> (Engine, PlaybackId) {
        let mut e = engine();
        e.select_fixture(1).unwrap();
        e.record_group(6).unwrap();
        let id = e.record_group_master(1, 8, 6).unwrap();
        (e, id)
    }

    fn deltas(d: &[String], kind: &str) -> Vec<Value> {
        d.iter()
            .map(|m| parse(m))
            .filter(|m| m["type"] == json!(kind))
            .collect()
    }

    #[test]
    fn grm_15_group_master_entry_has_kind_master_and_buttons() {
        let (e, id) = group_master_engine();
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        let m = entry(&v, u64::from(id));
        assert_eq!(m["kind"], json!("GroupMaster"));
        assert_eq!(
            m["master"],
            json!({"group":6,"mode":"Scale","scale":100,"enabled":true,"level":1.0})
        );
        assert_eq!(m["buttons"], json!(["None", "Flash"]));
        assert_eq!(m["level"], json!(1.0), "the live level is the fader");
    }

    #[test]
    fn ipc_29_a_group_master_fader_move_sends_only_the_small_delta() {
        let (mut e, id) = group_master_engine();
        let mut published = Published::new(&e, &[], &show_state());
        e.set_fader(id, 0.5, 0.0).unwrap();
        let d = published.diff(&e, &[], &show_state());
        assert!(deltas(&d, "playbacks").is_empty(), "{d:?}");
        let small = deltas(&d, "playback");
        assert_eq!(small.len(), 1, "{d:?}");
        assert_eq!(small[0]["level"], json!(0.5));

        // Disabling is a stored change: the whole list, with the locked level.
        let Some(Master::Group(mut m)) = e.playback(id).unwrap().master.clone() else {
            panic!()
        };
        m.enabled = false;
        e.change_playback(
            id,
            &PlaybackChange {
                master: Some(Master::Group(m)),
                ..Default::default()
            },
        )
        .unwrap();
        let d = published.diff(&e, &[], &show_state());
        let whole = deltas(&d, "playbacks");
        assert_eq!(whole.len(), 1, "{d:?}");
        let master = &entry(&whole[0], u64::from(id))["master"];
        assert_eq!(
            (&master["enabled"], &master["level"]),
            (&json!(false), &json!(0.5))
        );

        // A disabled master's fader moves the live level only.
        e.set_fader(id, 0.2, 0.0).unwrap();
        let d = published.diff(&e, &[], &show_state());
        assert!(deltas(&d, "playbacks").is_empty(), "{d:?}");
        assert_eq!(deltas(&d, "playback")[0]["level"], json!(0.2));
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert_eq!(entry(&v, u64::from(id))["master"]["level"], json!(0.5));
    }

    #[test]
    fn grm_15_group_master_commands_parse() {
        for good in [
            r#"{"v":0,"cmd":"master.record","page":1,"slot":8,"kind":"Group","group":6}"#,
            r#"{"v":0,"cmd":"playback.options","id":4,"master":{"mode":"Htp"}}"#,
            r#"{"v":0,"cmd":"playback.options","id":4,"master":{"scale":200}}"#,
            r#"{"v":0,"cmd":"playback.options","id":4,"master":{"enabled":false}}"#,
            r#"{"v":0,"cmd":"playback.options","id":4,"master":{"mode":"TakeOver","scale":1000,
                "enabled":true}}"#,
            r#"{"v":0,"cmd":"playback.options","id":4,"master":{"mode":"Limit"}}"#,
            r#"{"v":0,"cmd":"playback.options","id":4,"master":{"mode":"Scale"}}"#,
        ] {
            assert!(
                parse_command(good).is_ok(),
                "{good}: {:?}",
                parse_command(good)
            );
        }
        for bad in [
            r#"{"v":0,"cmd":"master.record","page":1,"slot":8,"kind":"Group"}"#,
            r#"{"v":0,"cmd":"master.record","page":1,"slot":8,"kind":"Group","group":"six"}"#,
            r#"{"v":0,"cmd":"master.record","page":1,"slot":8,"kind":"Group","group":6,"number":1}"#,
            r#"{"v":0,"cmd":"playback.options","id":4,"master":{"mode":"Dim"}}"#,
            r#"{"v":0,"cmd":"playback.options","id":4,"master":{"mode":"Unknown"}}"#,
            r#"{"v":0,"cmd":"playback.options","id":4,"master":{"level":0.5}}"#,
            r#"{"v":0,"cmd":"playback.options","id":4,"master":{"group":2}}"#,
            r#"{"v":0,"cmd":"playback.options","id":4,"master":{"enabled":"no"}}"#,
        ] {
            assert!(parse_command(bad).is_err(), "{bad}");
        }
    }

    // ---- undo (VS5-02: IPC-30/31) ----------------------------------------------

    #[test]
    fn ipc_30_undo_and_redo_parse() {
        assert_eq!(parse_command(r#"{"v":0,"cmd":"undo"}"#), Ok(Command::Undo));
        assert_eq!(parse_command(r#"{"v":0,"cmd":"redo"}"#), Ok(Command::Redo));
    }

    #[test]
    fn ipc_31_snapshot_has_the_undo_and_redo_labels() {
        let none = UndoInfo::default();
        let v = parse(&snapshot(
            &engine(),
            &[],
            &show_state(),
            &none,
            &serde_json::Value::Null,
        ));
        assert_eq!(v["undo"], json!({"undo": null, "redo": null}));
        let some = UndoInfo {
            undo: Some("Clear".into()),
            redo: Some("Wheel Pan".into()),
        };
        let v = parse(&snapshot(
            &engine(),
            &[],
            &show_state(),
            &some,
            &serde_json::Value::Null,
        ));
        assert_eq!(v["undo"], json!({"undo": "Clear", "redo": "Wheel Pan"}));
    }

    #[test]
    fn ipc_31_undo_delta_only_when_a_label_changes() {
        let e = engine();
        let mut published = Published::new(&e, &[], &show_state());
        assert_eq!(published.undo_delta(&UndoInfo::default()), None);
        let info = UndoInfo {
            undo: Some("Clear".into()),
            redo: None,
        };
        let d = published.undo_delta(&info).unwrap();
        assert_eq!(
            parse(&d),
            json!({"v":0,"type":"undo","undo":"Clear","redo":null})
        );
        assert_eq!(published.undo_delta(&info), None, "no change, no delta");
    }

    // ---- VS2-05: IPC-33 ----------------------------------------------------------------

    #[test]
    fn ipc_33_programmer_view_has_located_and_highlight_and_settings_are_published() {
        let mut e = engine_with_palettes();
        let v = parse(&snapshot(
            &e,
            &[],
            &show_state(),
            &UndoInfo::default(),
            &serde_json::Value::Null,
        ));
        assert_eq!(
            v["programmer_settings"],
            json!({"clear_precedence":"SelectionThenProgrammer","clear_ltp":"Release","lowlight_level":0.25})
        );
        assert_eq!(v["programmer"]["located"], json!({}));
        assert_eq!(v["programmer"]["highlight"], json!(false));

        let mut published = Published::new(&e, &[], &show_state());
        let first = e.fixtures().next().unwrap().id;
        e.select_fixture(first).unwrap();
        e.locate(&[cmdmx_core::attribute::AttributeGroup::I])
            .unwrap();
        e.set_highlight(true).unwrap();
        e.set_programmer_settings(cmdmx_core::programmer::ProgrammerSettingsChange {
            clear_ltp: Some(cmdmx_core::programmer::ClearLtp::Freeze),
            ..Default::default()
        })
        .unwrap();
        e.tick(0.025);
        let d: Vec<serde_json::Value> = published
            .diff(&e, &[], &show_state())
            .iter()
            .map(|m| parse(m))
            .collect();
        let prog = d.iter().find(|m| m["type"] == "programmer").unwrap();
        assert_eq!(prog["highlight"], json!(true));
        assert!(prog["located"][first.to_string()].is_array());
        let settings = d
            .iter()
            .find(|m| m["type"] == "programmer_settings")
            .unwrap();
        assert_eq!(settings["clear_ltp"], "Freeze");
    }
}

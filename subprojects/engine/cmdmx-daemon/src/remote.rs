//! RMT-01..08: the remote. A listener on every interface (a phone on the Wi-Fi), a login with a
//! salted password hash, and a WebSocket per phone that gets the patch and may rename and move
//! fixtures and ask for Highlight. The password is hashed on the app's client thread and never
//! reaches the tick; the tick only drains channels and `try_send`s, so a silent phone costs
//! nothing (IPC-07).

use std::collections::{HashMap, HashSet};
use std::io::{ErrorKind, Read};
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cmdmx_core::engine::Engine;
use serde_json::{json, Value};
use tungstenite::handshake::server::{ErrorResponse, Request as WsRequest, Response};
use tungstenite::http::StatusCode;
use tungstenite::{Error as WsError, Message, WebSocket};

use crate::ipc::{self, Command, PROTOCOL};

/// RMT-01: the port until the app sets another.
pub const DEFAULT_PORT: u16 = 7320;
/// IPC-07: messages a phone may have queued before it is dropped.
const QUEUE: usize = 64;
/// RMT-02: PBKDF2-HMAC-SHA256 rounds.
// ponytail: enough for a hash at rest on the owner's Mac; raise it if `state.json` ever leaves it.
const ROUNDS: u32 = 20_000;
/// RMT-05: wrong logins from one address before it is refused.
const MAX_TRIES: u32 = 5;
const LOCK_OUT: Duration = Duration::from_secs(30);
/// RMT-03: the whole HTTP head must arrive within this.
const HEAD_DEADLINE: Duration = Duration::from_secs(2);
/// A connection that has not logged in by now is closed.
const LOGIN_WAIT: Duration = Duration::from_secs(10);
/// RMT-02: the addresses shown in the app are looked up at most this often.
const ADDRESSES_EVERY: Duration = Duration::from_secs(2);
/// RMT-04: bumped by every `remote.set_login` the moment it arrives, before the (slow) hashing, so
/// the tokens and phones of the old login are dead at once and not only when the tick restarts.
static LOGIN_GENERATION: AtomicU64 = AtomicU64::new(0);
const WRONG: &str = "Wrong username or password";
const TOO_MANY: &str = "Too many tries, wait 30 s";

/// RMT-03: until the patch page (RMB-02).
const PAGE: &str = "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>CMDMX remote</title></head>\n<body><p>The patch page comes with RMB-02.</p></body></html>\n";

/// RMT-02: a login as it is kept: username, salt and hash, never the password.
#[derive(Debug, Clone, PartialEq)]
pub struct Login {
    pub username: String,
    pub salt: [u8; 16],
    pub hash: [u8; 32],
    pub port: u16,
}

/// What the app asks of the remote, after hashing (`Event::Remote`).
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Login(Login),
    Off,
}

/// From a phone's thread to the tick.
pub enum RemoteEvent {
    /// A phone logged in; the tick queues the snapshot first (RMT-06).
    Connected(SyncSender<String>),
    /// RMT-07: an allowed command; warnings go to `reply` as well as to the app.
    Command {
        cmd: Command,
        reply: SyncSender<String>,
    },
}

fn random<const N: usize>() -> Result<[u8; N], String> {
    let mut bytes = [0u8; N];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| format!("Remote: no random bytes for the login: {e}"))?;
    Ok(bytes)
}

fn hash(password: &str, salt: &[u8; 16]) -> [u8; 32] {
    pbkdf2::pbkdf2_hmac_array::<sha2::Sha256, 32>(password.as_bytes(), salt, ROUNDS)
}

/// RMT-02: a new salt every time, so the same password never gives the same hash twice.
pub fn hash_login(username: &str, password: &str, port: u16) -> Result<Login, String> {
    let salt = random::<16>()?;
    LOGIN_GENERATION.fetch_add(1, Ordering::SeqCst);
    Ok(Login {
        username: username.to_string(),
        salt,
        hash: hash(password, &salt),
        port,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let mut out = [0u8; N];
    if text.len() != 2 * N || !text.is_ascii() {
        return None;
    }
    for (slot, pair) in out.iter_mut().zip(text.as_bytes().chunks(2)) {
        *slot = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(out)
}

/// Equal without stopping at the first difference.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// RMT-04, 05: what the phone threads of one listener share. A new login makes a new one, so the
/// old tokens die with it. Only phone threads lock it, never the tick.
struct Auth {
    username: String,
    salt: [u8; 16],
    hash: [u8; 32],
    tokens: HashSet<String>,
    /// Wrong tries per address, and until when it is refused.
    tries: HashMap<IpAddr, (u32, Option<Instant>)>,
}

impl Auth {
    fn new(login: &Login) -> Auth {
        Auth {
            username: login.username.clone(),
            salt: login.salt,
            hash: login.hash,
            tokens: HashSet::new(),
            tries: HashMap::new(),
        }
    }

    /// The token for a right login, else the text to answer with.
    fn login(
        &mut self,
        ip: IpAddr,
        who: &Credentials,
        now: Instant,
    ) -> Result<String, &'static str> {
        let entry = self.tries.entry(ip).or_default();
        match entry.1 {
            Some(until) if now < until => return Err(TOO_MANY),
            Some(_) => *entry = (0, None),
            None => {}
        }
        let token = match who {
            Credentials::Password { username, password } => {
                let right =
                    *username == self.username && same(&hash(password, &self.salt), &self.hash);
                right
                    .then(|| random::<16>().ok().map(|t| hex(&t)))
                    .flatten()
            }
            Credentials::Token(token) => self.tokens.contains(token).then(|| token.clone()),
        };
        let entry = self.tries.entry(ip).or_default();
        let Some(token) = token else {
            entry.0 += 1;
            if entry.0 >= MAX_TRIES {
                entry.1 = Some(now + LOCK_OUT);
            }
            return Err(WRONG);
        };
        self.tries.remove(&ip);
        self.tokens.insert(token.clone());
        Ok(token)
    }
}

enum Credentials {
    Password { username: String, password: String },
    Token(String),
}

/// RMT-04: the first message of a phone, `None` when it is anything else.
fn credentials(text: &str) -> Option<Credentials> {
    let v: Value = serde_json::from_str(text).ok()?;
    if v.get("v")?.as_u64()? != PROTOCOL || v.get("cmd")?.as_str()? != "login" {
        return None;
    }
    let text = |key: &str| v.get(key)?.as_str().map(String::from);
    match text("token") {
        Some(token) => Some(Credentials::Token(token)),
        None => Some(Credentials::Password {
            username: text("username")?,
            password: text("password")?,
        }),
    }
}

/// RMT-03: whether a WebSocket upgrade may go ahead: exactly `/ws`, and an `Origin` that is the
/// page's own (`http://<Host>`), with `Host` on this listener's port and a name that is an
/// address literal, `localhost` or `*.local` (so a rebound DNS name does not pass).
fn origin_allowed(path: &str, host: Option<&str>, origin: Option<&str>, port: u16) -> bool {
    let (Some(host), Some(origin)) = (host, origin) else {
        return false;
    };
    let Some((name, host_port)) = host.rsplit_once(':') else {
        return false;
    };
    let literal = name.trim_start_matches('[').trim_end_matches(']');
    path == "/ws"
        && host_port == port.to_string()
        && origin == format!("http://{host}")
        && (literal.parse::<IpAddr>().is_ok()
            || name.eq_ignore_ascii_case("localhost")
            || name.to_ascii_lowercase().ends_with(".local"))
}

#[allow(clippy::result_large_err)] // the error type is dictated by tungstenite's callback trait
fn check_origin(port: u16, req: &WsRequest, resp: Response) -> Result<Response, ErrorResponse> {
    let header = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok());
    if origin_allowed(req.uri().path(), header("host"), header("origin"), port) {
        return Ok(resp);
    }
    let mut refusal = ErrorResponse::new(Some("not allowed".into()));
    *refusal.status_mut() = StatusCode::FORBIDDEN;
    Err(refusal)
}

/// What the threads of one listener share.
#[derive(Clone)]
struct Shared {
    auth: Arc<Mutex<Auth>>,
    events: Sender<RemoteEvent>,
    stop: Arc<AtomicBool>,
    count: Arc<AtomicUsize>,
    port: u16,
    /// `LOGIN_GENERATION` when this listener started.
    generation: u64,
}

impl Shared {
    /// A newer login has been set: this listener's tokens and phones are dead.
    fn stale(&self) -> bool {
        LOGIN_GENERATION.load(Ordering::SeqCst) != self.generation
    }
}

/// Decrements the client count when a phone's thread ends, however it ends.
struct Counted(Arc<AtomicUsize>);

impl Counted {
    fn new(count: &Arc<AtomicUsize>) -> Counted {
        count.fetch_add(1, Ordering::Relaxed);
        Counted(Arc::clone(count))
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// RMT-03: the request head, peeked (so a WebSocket handshake can still read it) until its blank
/// line; `None` when it is too long, too slow or not coming.
fn peek_head(stream: &TcpStream) -> Option<Vec<u8>> {
    let started = Instant::now();
    let mut buf = [0u8; 8192];
    while started.elapsed() < HEAD_DEADLINE {
        match stream.peek(&mut buf) {
            Ok(0) => return None,
            Ok(n) => {
                let seen = buf.get(..n)?;
                if let Some(end) = seen.windows(4).position(|w| w == b"\r\n\r\n") {
                    return seen.get(..end + 4).map(<[u8]>::to_vec);
                }
                if n == buf.len() {
                    return None;
                }
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return None,
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &str) {
    use std::io::Write;
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body.as_bytes()));
}

/// RMT-03: one connection: the page on `GET /`, a 404 elsewhere, a WebSocket on an upgrade.
#[allow(clippy::result_large_err)] // the handshake callback's error type is tungstenite's
fn serve_connection(mut stream: TcpStream, ip: IpAddr, shared: &Shared) {
    // The listener is non-blocking, and on macOS an accepted socket inherits that.
    if stream.set_nonblocking(false).is_err()
        || stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .is_err()
        || stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .is_err()
    {
        return;
    }
    let Some(head) = peek_head(&stream) else {
        return;
    };
    let text = String::from_utf8_lossy(&head);
    let mut lines = text.split("\r\n");
    let mut request = lines.next().unwrap_or_default().split(' ');
    let (method, path) = (request.next(), request.next());
    let upgrade = lines.any(|l| {
        l.split_once(':').is_some_and(|(k, v)| {
            k.eq_ignore_ascii_case("upgrade") && v.trim().eq_ignore_ascii_case("websocket")
        })
    });
    if upgrade {
        let port = shared.port;
        if let Ok(ws) = tungstenite::accept_hdr(stream, |r: &WsRequest, resp: Response| {
            check_origin(port, r, resp)
        }) {
            serve_phone(ws, ip, shared);
        }
        return;
    }
    // Take the head off the socket, so closing it does not reset the answer away.
    let mut taken = vec![0u8; head.len()];
    if stream.read_exact(&mut taken).is_err() {
        return;
    }
    match (method, path) {
        (Some("GET"), Some("/")) => {
            respond(&mut stream, "200 OK", "text/html; charset=utf-8", PAGE)
        }
        _ => respond(
            &mut stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            "Not found",
        ),
    }
}

fn login_reply(token: Result<&str, &str>) -> Message {
    let v = match token {
        Ok(t) => json!({"v": PROTOCOL, "type": "login", "token": t}),
        Err(e) => json!({"v": PROTOCOL, "type": "login", "error": e}),
    };
    Message::text(v.to_string())
}

/// RMT-07: `remote.highlight` is the phone's own; everything else goes through the app's parser.
fn parse_phone_command(text: &str) -> Result<Command, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("bad JSON: {e}"))?;
    if v.get("cmd").and_then(Value::as_str) != Some("remote.highlight") {
        return ipc::parse_command(text);
    }
    let id = match v.get("id") {
        Some(Value::Null) => None,
        Some(n) => Some(
            n.as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or("missing or invalid \"id\"")?,
        ),
        None => return Err("missing or invalid \"id\"".into()),
    };
    Ok(Command::RemoteHighlight { id })
}

fn is_idle(e: &WsError) -> bool {
    matches!(e, WsError::Io(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut))
}

/// RMT-04, 05, 07: the login, then the commands and the queues, in the shape of `serve_client`.
fn serve_phone(mut ws: WebSocket<TcpStream>, ip: IpAddr, shared: &Shared) {
    let tcp = ws.get_mut();
    if tcp
        .set_read_timeout(Some(Duration::from_millis(20)))
        .is_err()
        || tcp.set_write_timeout(Some(Duration::from_secs(2))).is_err()
    {
        return;
    }
    let started = Instant::now();
    let token = loop {
        if shared.stop.load(Ordering::Relaxed) || started.elapsed() > LOGIN_WAIT {
            return;
        }
        match ws.read() {
            Ok(Message::Text(text)) => {
                let Some(who) = credentials(text.as_str()) else {
                    return;
                };
                let answer = match shared.auth.lock() {
                    Ok(_) if shared.stale() => Err(WRONG),
                    Ok(mut auth) => auth.login(ip, &who, Instant::now()),
                    Err(_) => Err(WRONG),
                };
                match answer {
                    Ok(token) => break token,
                    Err(e) => {
                        let _ = ws.send(login_reply(Err(e)));
                        return;
                    }
                }
            }
            Ok(Message::Close(_)) => return,
            Ok(_) => {}
            Err(e) if is_idle(&e) => {}
            Err(_) => return,
        }
    };
    let _counted = Counted::new(&shared.count);
    let (tx, out) = sync_channel::<String>(QUEUE);
    let (reply, replies) = sync_channel::<String>(QUEUE);
    if ws.send(login_reply(Ok(&token))).is_err()
        || shared.events.send(RemoteEvent::Connected(tx)).is_err()
    {
        return;
    }
    loop {
        if shared.stop.load(Ordering::Relaxed) || shared.stale() {
            return;
        }
        match ws.read() {
            Ok(Message::Text(text)) => {
                let text = text.as_str();
                let answer = match parse_phone_command(text) {
                    Ok(
                        cmd @ (Command::PatchMove { .. }
                        | Command::PatchName { .. }
                        | Command::RemoteHighlight { .. }),
                    ) => {
                        let event = RemoteEvent::Command {
                            cmd,
                            reply: reply.clone(),
                        };
                        if shared.events.send(event).is_err() {
                            return;
                        }
                        None
                    }
                    Ok(_) => {
                        let name = serde_json::from_str::<Value>(text)
                            .ok()
                            .and_then(|v| v.get("cmd")?.as_str().map(String::from));
                        Some(format!(
                            "not allowed on the remote: {}",
                            name.unwrap_or_default()
                        ))
                    }
                    Err(e) => Some(e),
                };
                if let Some(e) = answer {
                    if ws.send(Message::text(ipc::error(&e))).is_err() {
                        return;
                    }
                }
            }
            Ok(Message::Close(_)) => return,
            Ok(_) => {}
            Err(e) if is_idle(&e) => {}
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
                // The tick dropped this phone (queue full, listener off).
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

/// A running listener: the flag that stops it and its phones, and the accept thread.
struct Listener {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

fn listen(
    port: u16,
    login: &Login,
    events: Sender<RemoteEvent>,
    count: Arc<AtomicUsize>,
) -> Result<Listener, String> {
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).map_err(|e| {
        if e.kind() == ErrorKind::AddrInUse {
            format!("Remote: port {port} is in use")
        } else {
            format!("Remote: cannot listen on port {port}: {e}")
        }
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Remote: cannot listen on port {port}: {e}"))?;
    let stop = Arc::new(AtomicBool::new(false));
    let shared = Shared {
        auth: Arc::new(Mutex::new(Auth::new(login))),
        events,
        stop: Arc::clone(&stop),
        count,
        port,
        generation: LOGIN_GENERATION.load(Ordering::SeqCst),
    };
    let thread = std::thread::Builder::new()
        .name("remote-listener".into())
        .spawn(move || {
            while !shared.stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, peer)) => {
                        let shared = shared.clone();
                        // A failed spawn only loses this one connection.
                        let _ = std::thread::Builder::new()
                            .name("remote-client".into())
                            .spawn(move || {
                                // A panic closes only this phone.
                                if catch_unwind(AssertUnwindSafe(|| {
                                    serve_connection(stream, peer.ip(), &shared);
                                }))
                                .is_err()
                                {
                                    eprintln!("remote: client thread panicked; connection closed");
                                }
                            });
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(25)),
                }
            }
        })
        .map_err(|e| format!("Remote: cannot start the listener thread: {e}"))?;
    Ok(Listener {
        stop,
        thread: Some(thread),
    })
}

/// RMT-02: the Mac's IPv4 addresses that are up, loopback left out.
#[cfg(unix)]
fn local_addresses() -> Vec<String> {
    let mut found = Vec::new();
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: `getifaddrs` fills `head` with a list that is only read here, each `ifa_addr` is
    // checked for null and for AF_INET before it is read as a `sockaddr_in`, and the list is
    // always handed back to `freeifaddrs`.
    unsafe {
        if libc::getifaddrs(&mut head) != 0 {
            return found;
        }
        let mut at = head;
        while !at.is_null() {
            let ifa = &*at;
            let up = ifa.ifa_flags & (libc::IFF_UP as libc::c_uint) != 0;
            if up
                && !ifa.ifa_addr.is_null()
                && i32::from((*ifa.ifa_addr).sa_family) == libc::AF_INET
            {
                let sin = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
                if !ip.is_loopback() {
                    found.push(ip.to_string());
                }
            }
            at = ifa.ifa_next;
        }
        libc::freeifaddrs(head);
    }
    found
}

#[cfg(not(unix))]
fn local_addresses() -> Vec<String> {
    Vec::new()
}

/// RMT-06: what a phone sees: the show, the lock, the highlighted fixture and the patch in number
/// order. `parked` is false until PRK-01.
pub fn snapshot(engine: &Engine, show: &str, locked: bool) -> String {
    let mut fixtures: Vec<_> = engine.fixtures().collect();
    fixtures.sort_by_key(|f| (f.number, f.id));
    let fixtures: Vec<Value> = fixtures
        .into_iter()
        .map(|f| {
            let mode = engine.fixture_mode(f.id);
            json!({
                "id": f.id,
                "number": f.number,
                "name": f.name,
                "profile": f.profile,
                "mode": mode.map_or("", |m| m.name.as_str()),
                "universe": f.universe,
                "address": f.address,
                "footprint": mode.map_or(0, |m| m.footprint),
                "parked": false,
            })
        })
        .collect();
    // Highlight counts as the phone's when exactly one whole fixture is selected.
    let highlight = match engine.programmer().items() {
        [one] if one.cell.is_none() && engine.highlight() => Some(one.fixture),
        _ => None,
    };
    json!({"v": PROTOCOL, "type": "snapshot", "show": show, "locked": locked,
        "highlight": highlight, "fixtures": fixtures})
    .to_string()
}

/// The tick's side of the remote: the saved login, the running listener, the phones' queues.
pub struct Remote {
    state_file: PathBuf,
    login: Option<Login>,
    listener: Option<Listener>,
    events_tx: Sender<RemoteEvent>,
    events: Receiver<RemoteEvent>,
    phones: Vec<SyncSender<String>>,
    count: Arc<AtomicUsize>,
    addresses: Vec<String>,
    addresses_at: Option<Instant>,
    sent_info: Option<Value>,
    sent_snapshot: String,
    /// A login or off came in: the app hears about it even when nothing in `info` changed.
    forced: bool,
}

impl Remote {
    /// RMT-01: read the `remote` key of `state.json` and, when it was on, start the listener.
    /// The strings are warnings; a failed start leaves the saved `on` as it was.
    pub fn load(state_file: PathBuf) -> (Remote, Vec<String>) {
        let (events_tx, events) = channel();
        let mut remote = Remote {
            state_file,
            login: None,
            listener: None,
            events_tx,
            events,
            phones: Vec::new(),
            count: Arc::default(),
            addresses: Vec::new(),
            addresses_at: None,
            sent_info: None,
            sent_snapshot: String::new(),
            forced: false,
        };
        let saved: Option<Value> = std::fs::read_to_string(&remote.state_file)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.get("remote").cloned());
        let mut warnings = Vec::new();
        if let Some(saved) = saved {
            let text = |key: &str| saved.get(key).and_then(Value::as_str);
            remote.login = (|| {
                Some(Login {
                    username: text("username")?.to_string(),
                    salt: unhex(text("salt")?)?,
                    hash: unhex(text("hash")?)?,
                    port: u16::try_from(saved.get("port")?.as_u64()?).ok()?,
                })
            })();
            if saved.get("on") == Some(&json!(true)) {
                if let Err(w) = remote.start() {
                    warnings.push(w);
                }
            }
        }
        remote.refresh(Instant::now());
        remote.sent_info = Some(remote.info());
        (remote, warnings)
    }

    fn start(&mut self) -> Result<(), String> {
        self.stop();
        let login = self.login.as_ref().ok_or("Remote: no login is set")?;
        let listener = listen(
            login.port,
            login,
            self.events_tx.clone(),
            Arc::clone(&self.count),
        )?;
        self.listener = Some(listener);
        Ok(())
    }

    /// Close the port and every phone. The accept loop notices within 25 ms; waiting for it keeps
    /// the port free for a restart on the same number.
    // ponytail: the tick waits up to 25 ms, once per operator action.
    fn stop(&mut self) {
        if let Some(mut l) = self.listener.take() {
            l.stop.store(true, Ordering::Relaxed);
            if let Some(t) = l.thread.take() {
                let _ = t.join();
            }
        }
        self.phones.clear();
    }

    fn save(&self, on: bool) -> Result<(), String> {
        let Some(l) = &self.login else { return Ok(()) };
        let value = json!({"on": on, "username": l.username, "salt": hex(&l.salt),
            "hash": hex(&l.hash), "port": l.port});
        crate::shows::update_state(&self.state_file, "remote", value)
            .map_err(|e| format!("Remote: cannot save the login: {e}"))
    }

    /// RMT-01: a new login restarts the listener (and so closes the phones); off stops it.
    /// The warning, if any, is for the app.
    pub fn request(&mut self, req: Request) -> Option<String> {
        self.forced = true;
        self.addresses_at = None;
        let started = match req {
            Request::Login(login) => {
                self.login = Some(login);
                self.start()
            }
            Request::Off => {
                self.stop();
                Ok(())
            }
        };
        let saved = self.save(started.is_ok() && self.listener.is_some());
        started.err().or(saved.err())
    }

    pub fn next_event(&mut self) -> Option<RemoteEvent> {
        self.events.try_recv().ok()
    }

    /// RMT-06: the snapshot goes first; a phone that cannot take even that is not kept.
    pub fn connect(&mut self, tx: SyncSender<String>, snapshot: &str) {
        if tx.try_send(snapshot.to_string()).is_ok() {
            self.phones.push(tx);
        }
    }

    pub fn has_phones(&self) -> bool {
        !self.phones.is_empty()
    }

    /// RMT-06: to every phone when it differs from the last one sent; a full queue drops the phone.
    // ponytail: the whole snapshot is rebuilt per tick while phones are connected; diff the
    // fixtures if it ever shows in a profile.
    pub fn publish(&mut self, snapshot: String) {
        if snapshot != self.sent_snapshot {
            self.phones
                .retain(|tx| tx.try_send(snapshot.clone()).is_ok());
            self.sent_snapshot = snapshot;
        }
    }

    fn refresh(&mut self, now: Instant) {
        if self.listener.is_none() {
            self.addresses.clear();
        } else if self
            .addresses_at
            .is_none_or(|at| now.duration_since(at) >= ADDRESSES_EVERY)
        {
            self.addresses = local_addresses();
            self.addresses_at = Some(now);
        }
    }

    /// RMT-02: exactly these five keys; never the salt, the hash or the password.
    pub fn info(&self) -> Value {
        json!({
            "on": self.listener.is_some(),
            "username": self.login.as_ref().map(|l| l.username.as_str()),
            "port": self.login.as_ref().map_or(DEFAULT_PORT, |l| l.port),
            "addresses": self.addresses,
            "clients": self.count.load(Ordering::Relaxed),
        })
    }

    /// RMT-02: the `remote` delta, once per change.
    pub fn delta(&mut self, now: Instant) -> Option<String> {
        self.refresh(now);
        let info = self.info();
        if !std::mem::take(&mut self.forced) && self.sent_info.as_ref() == Some(&info) {
            return None;
        }
        let msg = json!({"v": PROTOCOL, "type": "remote", "remote": info}).to_string();
        self.sent_info = Some(info);
        Some(msg)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn rmt_03_origin_is_the_pages_own_on_this_port_and_a_local_name() {
        let ok = |path, host, origin| origin_allowed(path, host, origin, 7320);
        let (host, origin) = (Some("192.168.1.5:7320"), Some("http://192.168.1.5:7320"));
        assert!(ok("/ws", host, origin));
        assert!(ok(
            "/ws",
            Some("localhost:7320"),
            Some("http://localhost:7320")
        ));
        assert!(ok(
            "/ws",
            Some("mac.local:7320"),
            Some("http://mac.local:7320")
        ));
        assert!(ok("/ws", Some("[::1]:7320"), Some("http://[::1]:7320")));
        assert!(!ok("/", host, origin), "only /ws");
        assert!(!ok("/ws", host, None), "no Origin");
        assert!(!ok("/ws", host, Some("http://evil.example")));
        assert!(!ok(
            "/ws",
            Some("evil.example:7320"),
            Some("http://evil.example:7320")
        ));
        assert!(!ok(
            "/ws",
            Some("192.168.1.5:80"),
            Some("http://192.168.1.5:80")
        ));
        assert!(!ok("/ws", Some("192.168.1.5"), Some("http://192.168.1.5")));
    }

    #[test]
    fn rmt_02_hash_verifies_and_salts_differ() {
        let a = hash_login("aernout", "kerst2026", 7320).unwrap();
        let b = hash_login("aernout", "kerst2026", 7320).unwrap();
        assert_ne!(a.salt, b.salt);
        assert_ne!(a.hash, b.hash);
        assert!(same(&hash("kerst2026", &a.salt), &a.hash));
        assert!(!same(&hash("kerst2027", &a.salt), &a.hash));
        assert_eq!(unhex::<16>(&hex(&a.salt)), Some(a.salt));
    }

    #[test]
    fn rmt_05_five_wrong_tries_lock_an_address_out_and_a_right_one_resets() {
        let mut auth = Auth::new(&hash_login("u", "p", 7320).unwrap());
        let ip: IpAddr = "10.0.0.2".parse().unwrap();
        let wrong = Credentials::Password {
            username: "u".into(),
            password: "x".into(),
        };
        let right = Credentials::Password {
            username: "u".into(),
            password: "p".into(),
        };
        let now = Instant::now();
        for _ in 0..4 {
            assert_eq!(auth.login(ip, &wrong, now), Err(WRONG));
        }
        let token = auth.login(ip, &right, now).unwrap();
        assert_eq!(
            auth.login(ip, &Credentials::Token(token), now).map(|_| ()),
            Ok(())
        );
        for _ in 0..5 {
            assert_eq!(auth.login(ip, &wrong, now), Err(WRONG));
        }
        assert_eq!(auth.login(ip, &right, now), Err(TOO_MANY));
        assert!(auth.login(ip, &right, now + LOCK_OUT).is_ok());
    }
}

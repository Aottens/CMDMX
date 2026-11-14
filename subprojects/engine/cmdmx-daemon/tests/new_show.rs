//! A new show that keeps the patch (SHW-19) or the looks (SHW-20), a copy (SHW-21), this Mac's output (SHW-24), renaming and deleting shows (SHW-22) and the report (SHW-25), on the real binary: the switch, the files and the
//! history live in the tick. What the new show holds is `cmdmx-core/tests/showfile.rs`.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

/// Kills the daemon when the test ends, passed or not.
struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn connect(port: u16) -> WebSocket<TcpStream> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
            stream
                .set_read_timeout(Some(Duration::from_secs(15)))
                .unwrap();
            return tungstenite::client(format!("ws://127.0.0.1:{port}/"), stream)
                .unwrap()
                .0;
        }
        assert!(Instant::now() < deadline, "daemon never listened on {port}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn recv(ws: &mut WebSocket<TcpStream>) -> Value {
    loop {
        if let Message::Text(t) = ws.read().unwrap() {
            return serde_json::from_str(&t).unwrap();
        }
    }
}

fn send(ws: &mut WebSocket<TcpStream>, mut cmd: Value) {
    cmd["v"] = json!(1);
    ws.send(Message::Text(cmd.to_string().into())).unwrap();
}

fn until(ws: &mut WebSocket<TcpStream>, what: &str, pred: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "never saw {what}");
        let m = recv(ws);
        if pred(&m) {
            return m;
        }
    }
}

fn len(v: &Value) -> usize {
    v.as_array().unwrap().len()
}

#[test]
fn shw_19_new_show_keeps_the_patch_saves_the_old_show_and_empties_the_history() {
    let dir = std::env::temp_dir().join(format!("cmdmx-shw19-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shows = dir.join("Shows");
    let port = free_port();
    let _daemon = Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            .env("HOME", &dir)
            .args(["--no-artnet", "--no-sacn", "--no-serial"])
            .args(["--ipc-port", &port.to_string()])
            .arg("--show-dir")
            .arg(&shows)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut app = connect(port);
    let demo = recv(&mut app);
    assert_eq!(demo["type"], "snapshot");
    assert_eq!(demo["show"]["name"], "Demo");
    assert!(len(&demo["fixtures"]) > 0);
    assert!(len(&demo["playbacks"]) > 0);

    // A change the autosave has not written yet, and an undo step.
    let fixture = demo["fixtures"][0]["id"].clone();
    send(
        &mut app,
        json!({"cmd": "programmer.select", "fixture": fixture}),
    );
    send(&mut app, json!({"cmd": "group.record", "group": 77}));
    until(&mut app, "the undo label", |m| {
        m["type"] == "undo" && !m["undo"].is_null()
    });

    send(
        &mut app,
        json!({"cmd": "show.new", "name": "Kerk kerst", "keep": "patch"}),
    );
    let new = until(&mut app, "the new show", |m| {
        m["type"] == "snapshot" && m["show"]["name"] == "Kerk kerst"
    });
    assert_eq!(new["fixtures"], demo["fixtures"]);
    assert_eq!(len(&new["groups"]), 0);
    assert_eq!(len(&new["palettes"]), 0);
    assert_eq!(len(&new["playbacks"]), 0);
    assert_eq!(new["page"]["number"], 1);
    assert!(new["undo"]["undo"].is_null(), "UNDO-13: {}", new["undo"]);

    // The old file holds the old show with the last change, the new file exists at once.
    let read = |name: &str| -> Value {
        let path = shows.join(format!("{name}.cmdmx.json"));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match std::fs::read_to_string(&path) {
                Ok(text) => return serde_json::from_str(&text).unwrap(),
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => panic!("{}: {e}", path.display()),
            }
        }
    };
    let old = read("Demo");
    assert!(old["groups"]
        .as_array()
        .unwrap()
        .iter()
        .any(|g| g["id"] == 77));
    assert!(len(&old["playbacks"]["playbacks"]) > 0);
    assert_eq!(
        len(&read("Kerk kerst")["patch"]["fixtures"]),
        len(&demo["fixtures"])
    );

    // SHW-15: an existing name is refused and the show keeps running.
    send(
        &mut app,
        json!({"cmd": "show.new", "name": "Demo", "keep": "patch"}),
    );
    let w = until(&mut app, "the refusal", |m| m["type"] == "warning");
    assert!(
        w["message"].as_str().unwrap().contains("already exists"),
        "{w}"
    );

    // Without `keep` it stays SHW-15: an empty patch.
    send(&mut app, json!({"cmd": "show.new", "name": "Leeg"}));
    let empty = until(&mut app, "the empty show", |m| {
        m["type"] == "snapshot" && m["show"]["name"] == "Leeg"
    });
    assert_eq!(len(&empty["fixtures"]), 0);
}

#[test]
fn shw_20_next_weeks_show_keeps_groups_and_palettes_and_empties_the_playbacks() {
    let dir = std::env::temp_dir().join(format!("cmdmx-shw20-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let port = free_port();
    let _daemon = Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            .env("HOME", &dir)
            .args(["--no-artnet", "--no-sacn", "--no-serial"])
            .args(["--ipc-port", &port.to_string()])
            .arg("--show-dir")
            .arg(dir.join("Shows"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut app = connect(port);
    let demo = recv(&mut app);
    assert_eq!(demo["show"]["name"], "Demo");
    assert!(len(&demo["groups"]) > 0);
    assert!(len(&demo["playbacks"]) > 0);
    // A palette and a group, so there is something to keep.
    send(
        &mut app,
        json!({"cmd": "programmer.select", "fixture": 201}),
    );
    send(
        &mut app,
        json!({"cmd": "programmer.set", "attribute": "White", "value": 1.0}),
    );
    send(
        &mut app,
        json!({"cmd": "palette.record", "window": "Colours", "handle": 1}),
    );
    let palettes = until(&mut app, "the palette", |m| {
        m["type"] == "palettes" && len(&m["palettes"]) > 0
    })["palettes"]
        .clone();
    send(&mut app, json!({"cmd": "group.record", "group": 77}));
    until(&mut app, "the group", |m| {
        m["type"] == "groups"
            && m["groups"]
                .as_array()
                .unwrap()
                .iter()
                .any(|g| g["id"] == 77)
    });

    send(
        &mut app,
        json!({"cmd": "show.new", "name": "Kerk 16 nov", "keep": "looks"}),
    );
    let new = until(&mut app, "the new show", |m| {
        m["type"] == "snapshot" && m["show"]["name"] == "Kerk 16 nov"
    });
    assert_eq!(new["fixtures"], demo["fixtures"]);
    assert_eq!(len(&new["groups"]), len(&demo["groups"]) + 1);
    assert_eq!(new["palettes"], palettes);
    assert_eq!(len(&new["playbacks"]), 0);
    assert_eq!(new["page"]["number"], 1);
    assert!(new["undo"]["undo"].is_null(), "UNDO-13: {}", new["undo"]);

    // Any other value is refused and the show keeps running.
    send(
        &mut app,
        json!({"cmd": "show.new", "name": "Kerk 23 nov", "keep": "playbacks"}),
    );
    let e = until(&mut app, "the refusal", |m| m["type"] == "error");
    assert!(e["message"].as_str().unwrap().contains("keep"), "{e}");
}

#[test]
fn shw_21_save_copy_writes_a_copy_and_stays_in_the_open_show() {
    let dir = std::env::temp_dir().join(format!("cmdmx-shw21-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shows = dir.join("Shows");
    let port = free_port();
    let _daemon = Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            .env("HOME", &dir)
            .args(["--no-artnet", "--no-sacn", "--no-serial"])
            .args(["--ipc-port", &port.to_string()])
            .arg("--show-dir")
            .arg(&shows)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut app = connect(port);
    let demo = recv(&mut app);
    assert_eq!(demo["show"]["name"], "Demo");
    // A change the autosave has not written yet, and one undo step.
    let fixture = demo["fixtures"][0]["id"].clone();
    send(
        &mut app,
        json!({"cmd": "programmer.select", "fixture": fixture}),
    );
    send(&mut app, json!({"cmd": "group.record", "group": 77}));
    until(&mut app, "the undo label", |m| {
        m["type"] == "undo" && !m["undo"].is_null()
    });

    send(
        &mut app,
        json!({"cmd": "show.save_copy", "name": "Demo kopie"}),
    );
    // Every client hears that it is there and gets the new list, unasked.
    let n = until(&mut app, "the notice", |m| m["type"] == "notice");
    assert_eq!(n["message"], "Saved a copy as Demo kopie", "{n}");
    let list = until(&mut app, "the new list", |m| m["type"] == "show.list");
    assert!(
        list["shows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["name"] == "Demo kopie"),
        "{list}"
    );
    // The copy holds the change.
    let path = shows.join("Demo kopie.cmdmx.json");
    let deadline = Instant::now() + Duration::from_secs(5);
    let copy: Value = loop {
        match std::fs::read_to_string(&path) {
            Ok(text) => break serde_json::from_str(&text).unwrap(),
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => panic!("{}: {e}", path.display()),
        }
    };
    assert_eq!(copy["meta"]["name"], "Demo kopie");
    assert!(copy["groups"]
        .as_array()
        .unwrap()
        .iter()
        .any(|g| g["id"] == 77));

    // The open show stays: no switch, its undo history is kept.
    send(&mut app, json!({"cmd": "undo"}));
    let m = until(&mut app, "the undo or a switch", |m| {
        m["type"] == "snapshot" || (m["type"] == "undo" && m["redo"] == "Record group 77")
    });
    assert_eq!(m["type"], "undo", "no new snapshot after a copy: {m}");
    send(&mut app, json!({"cmd": "show.list"}));
    let list = until(&mut app, "the list", |m| m["type"] == "show.list");
    let names: Vec<&str> = list["shows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"Demo kopie"), "{names:?}");
    // SHW-15/09: a taken or invalid name is refused, as a warning.
    for (name, why) in [("Demo kopie", "already exists"), ("a/b", "not a show name")] {
        send(&mut app, json!({"cmd": "show.save_copy", "name": name}));
        let w = until(&mut app, "the refusal", |m| m["type"] == "warning");
        assert!(w["message"].as_str().unwrap().contains(why), "{name}: {w}");
    }
    let now = recv(&mut connect(port));
    assert_eq!(now["show"]["name"], "Demo");
}

/// (id, line, name) of each output node in a snapshot or `outputs` delta.
fn nodes(m: &Value) -> Vec<(u64, u64, String)> {
    m["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| {
            let name = o["name"].as_str().unwrap_or("").to_string();
            (o["id"].as_u64().unwrap(), o["line"].as_u64().unwrap(), name)
        })
        .collect()
}

fn named(m: &Value, name: &str) -> bool {
    nodes(m).iter().any(|n| n.2 == name)
}

#[test]
fn shw_24_open_keeps_this_macs_output_when_asked_and_new_always_does() {
    let dir = std::env::temp_dir().join(format!("cmdmx-shw24-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shows = dir.join("Shows");
    let port = free_port();
    let _daemon = Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            .env("HOME", &dir)
            .args(["--no-artnet", "--no-sacn", "--no-serial"])
            .args(["--ipc-port", &port.to_string()])
            .arg("--show-dir")
            .arg(&shows)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut app = connect(port);
    let demo = recv(&mut app);
    assert_eq!(demo["show"]["name"], "Demo");
    send(
        &mut app,
        json!({"cmd": "output.add", "line": 3, "kind": "artnet", "name": "Kerk"}),
    );
    let running = until(&mut app, "the Kerk node", |m| {
        m["type"] == "outputs" && named(m, "Kerk")
    });

    // New takes the running nodes, also without `keep`.
    send(&mut app, json!({"cmd": "show.new", "name": "Thuis"}));
    let thuis = until(&mut app, "the new show", |m| {
        m["type"] == "snapshot" && m["show"]["name"] == "Thuis"
    });
    assert_eq!(nodes(&thuis), nodes(&running));
    assert_eq!(len(&thuis["fixtures"]), 0);
    send(
        &mut app,
        json!({"cmd": "output.add", "line": 4, "kind": "artnet", "name": "Studio"}),
    );
    until(&mut app, "the Studio node", |m| {
        m["type"] == "outputs" && named(m, "Studio")
    });

    // Without `output` an Open uses the show's own nodes, as before.
    send(&mut app, json!({"cmd": "show.open", "name": "Demo"}));
    let demo = until(&mut app, "Demo", |m| {
        m["type"] == "snapshot" && m["show"]["name"] == "Demo"
    });
    assert_eq!(nodes(&demo), nodes(&running));

    // With `"output":"keep"` the running nodes stay, and the next save writes them into Thuis.
    send(
        &mut app,
        json!({"cmd": "show.open", "name": "Thuis", "output": "keep"}),
    );
    let thuis = until(&mut app, "Thuis", |m| {
        m["type"] == "snapshot" && m["show"]["name"] == "Thuis"
    });
    assert_eq!(nodes(&thuis), nodes(&running));
    assert!(!named(&thuis, "Studio"));
    send(&mut app, json!({"cmd": "show.open", "name": "Demo"}));
    until(&mut app, "Demo again", |m| {
        m["type"] == "snapshot" && m["show"]["name"] == "Demo"
    });
    let path = shows.join("Thuis.cmdmx.json");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let file: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let names: Vec<&str> = file["output"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["name"].as_str().unwrap_or(""))
            .collect();
        if !names.contains(&"Studio") {
            assert!(names.contains(&"Kerk"), "{names:?}");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Thuis still has its own nodes: {names:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }

    // Any other value is refused.
    send(
        &mut app,
        json!({"cmd": "show.open", "name": "Thuis", "output": "file"}),
    );
    let e = until(&mut app, "the refusal", |m| m["type"] == "error");
    assert!(e["message"].as_str().unwrap().contains("output"), "{e}");
}

/// The names in a `show.list`.
fn names(list: &Value) -> Vec<String> {
    list["shows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect()
}

/// A file the saver writes soon, as JSON.
fn read_soon(path: &std::path::Path, ok: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(v) = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .filter(&ok)
        {
            return v;
        }
        assert!(Instant::now() < deadline, "{} never came", path.display());
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn shw_22_rename_and_delete_shows_and_rename_the_open_one() {
    let dir = std::env::temp_dir().join(format!("cmdmx-shw22-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shows = dir.join("Shows");
    let port = free_port();
    let _daemon = Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            .env("HOME", &dir)
            .args(["--no-artnet", "--no-sacn", "--no-serial"])
            .args(["--ipc-port", &port.to_string()])
            .arg("--show-dir")
            .arg(&shows)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut app = connect(port);
    let demo = recv(&mut app);
    assert_eq!(demo["show"]["name"], "Demo");

    // show.list answers the folder, for Show in Finder.
    send(&mut app, json!({"cmd": "show.list"}));
    let list = until(&mut app, "the list", |m| m["type"] == "show.list");
    assert_eq!(list["dir"], shows.to_str().unwrap(), "{list}");

    // Two shows that are not open.
    for name in ["Kerk oud", "Zondag"] {
        send(&mut app, json!({"cmd": "show.save_copy", "name": name}));
        until(&mut app, "the copy", |m| {
            m["type"] == "show.list" && names(m).contains(&name.to_string())
        });
    }
    std::fs::write(shows.join("Kerk oud.cmdmx.json.bak"), "bak").unwrap();

    // Rename one: the file and its .bak move, every client gets the new list.
    send(
        &mut app,
        json!({"cmd": "show.rename", "name": "Kerk oud", "to": "Kerk archief"}),
    );
    let list = until(&mut app, "the renamed list", |m| {
        m["type"] == "show.list" && names(m).contains(&"Kerk archief".to_string())
    });
    assert!(!names(&list).contains(&"Kerk oud".to_string()), "{list}");
    assert!(shows.join("Kerk archief.cmdmx.json.bak").is_file());
    assert!(!shows.join("Kerk oud.cmdmx.json.bak").exists());

    // Refusals are warnings (IPC-06); the open show is never deleted.
    for (cmd, why) in [
        (
            json!({"cmd": "show.rename", "name": "Kerk archief", "to": "zondag"}),
            "already exists",
        ),
        (
            json!({"cmd": "show.rename", "name": "Kerk archief", "to": "a/b"}),
            "not a show name",
        ),
        (
            json!({"cmd": "show.rename", "name": "Nope", "to": "Iets"}),
            "no show named Nope",
        ),
        (
            json!({"cmd": "show.delete", "name": "Nope"}),
            "no show named Nope",
        ),
        (
            json!({"cmd": "show.delete", "name": "Demo"}),
            "Demo is open: open another show first",
        ),
    ] {
        send(&mut app, cmd.clone());
        let w = until(&mut app, "the refusal", |m| m["type"] == "warning");
        assert!(w["message"].as_str().unwrap().contains(why), "{cmd}: {w}");
    }

    // Delete one: to the Trash (`<show-dir>/Trash` with --show-dir), with its .bak.
    send(
        &mut app,
        json!({"cmd": "show.delete", "name": "Kerk archief"}),
    );
    until(&mut app, "the list without it", |m| {
        m["type"] == "show.list" && !names(m).contains(&"Kerk archief".to_string())
    });
    assert!(shows.join("Trash/Kerk archief.cmdmx.json").is_file());
    assert!(shows.join("Trash/Kerk archief.cmdmx.json.bak").is_file());
    assert!(!shows.join("Kerk archief.cmdmx.json").exists());

    // Rename the open show with a change not saved yet: written, renamed, and it carries on.
    let fixture = demo["fixtures"][0]["id"].clone();
    send(
        &mut app,
        json!({"cmd": "programmer.select", "fixture": fixture}),
    );
    send(&mut app, json!({"cmd": "group.record", "group": 77}));
    until(&mut app, "the undo label", |m| {
        m["type"] == "undo" && !m["undo"].is_null()
    });
    send(
        &mut app,
        json!({"cmd": "show.rename", "name": "Demo", "to": "Kerk"}),
    );
    let m = until(&mut app, "the show delta", |m| m["type"] == "show");
    assert_eq!(m["name"], "Kerk", "{m}");
    let saved = read_soon(&shows.join("Kerk.cmdmx.json"), |v| {
        v["groups"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["id"] == 77)
    });
    assert_eq!(saved["meta"]["name"], "Kerk");
    assert!(!shows.join("Demo.cmdmx.json").exists());
    read_soon(&shows.join("state.json"), |v| v["last_show"] == "Kerk");
    // The same show under a new name: its undo history stays, no new snapshot.
    send(&mut app, json!({"cmd": "undo"}));
    let m = until(&mut app, "the undo or a switch", |m| {
        m["type"] == "snapshot" || (m["type"] == "undo" && m["redo"] == "Record group 77")
    });
    assert_eq!(m["type"], "undo", "no new snapshot after a rename: {m}");
    let now = recv(&mut connect(port));
    assert_eq!(now["show"]["name"], "Kerk");
}

/// The first `<prefix>….cmdmx.json` in `dir`, once it is there.
fn find_soon(dir: &std::path::Path, prefix: &str) -> std::path::PathBuf {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let found = std::fs::read_dir(dir).unwrap().flatten().find_map(|e| {
            let name = e.file_name().into_string().ok()?;
            (name.starts_with(prefix) && name.ends_with(".cmdmx.json")).then(|| e.path())
        });
        if let Some(p) = found {
            return p;
        }
        assert!(
            Instant::now() < deadline,
            "no {prefix}… in {}",
            dir.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn says(m: &Value, what: &str) -> bool {
    m["type"] == "warning" && m["message"].as_str().unwrap().contains(what)
}

#[test]
fn shw_23_a_file_changed_outside_is_kept_and_open_of_the_open_show_reloads_it() {
    let dir = std::env::temp_dir().join(format!("cmdmx-shw23-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shows = dir.join("Shows");
    let port = free_port();
    let _daemon = Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            .env("HOME", &dir)
            .args(["--no-artnet", "--no-sacn", "--no-serial"])
            .args(["--ipc-port", &port.to_string()])
            .arg("--show-dir")
            .arg(&shows)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut app = connect(port);
    let demo = recv(&mut app);
    let demo_fixtures = len(&demo["fixtures"]);
    assert!(demo_fixtures > 0);
    // An empty show Kerk; the Demo is written as it is left.
    send(&mut app, json!({"cmd": "show.new", "name": "Kerk"}));
    let snap = until(&mut app, "Kerk", |m| {
        m["type"] == "snapshot" && m["show"]["name"] == "Kerk"
    });
    assert_eq!(len(&snap["fixtures"]), 0);
    let kerk = shows.join("Kerk.cmdmx.json");
    read_soon(&kerk, |_| true);
    let demo_file = read_soon(&shows.join("Demo.cmdmx.json"), |_| true);

    // Unchanged: Open of the open show is refused, with the hint.
    send(&mut app, json!({"cmd": "show.open", "name": "Kerk"}));
    until(&mut app, "the refusal", |m| {
        says(
            m,
            "Kerk is already open: its file has not changed outside CMDMX",
        )
    });

    // The Demo copied over Kerk in Finder: show.list tells, Open loads it from disk.
    std::fs::write(&kerk, demo_file.to_string()).unwrap();
    send(&mut app, json!({"cmd": "show.list"}));
    until(&mut app, "the notice", |m| {
        says(
            m,
            "Kerk.cmdmx.json was changed outside CMDMX: Open Kerk to load it",
        )
    });
    send(&mut app, json!({"cmd": "show.open", "name": "Kerk"}));
    let snap = until(&mut app, "the reload", |m| m["type"] == "snapshot");
    assert_eq!(snap["show"]["name"], "Kerk");
    assert_eq!(len(&snap["fixtures"]), demo_fixtures, "loaded from disk");
    until(&mut app, "which file holds the running show", |m| {
        says(m, "kept as Kerk before open 20")
    });
    let before = find_soon(&shows, "Kerk before open ");
    let kept = read_soon(&before, |_| true);
    assert_eq!(
        len(&kept["patch"]["fixtures"]),
        0,
        "the show that ran, kept"
    );

    // Changed outside again, then a change in CMDMX: the autosave keeps the file aside first.
    std::fs::write(&kerk, kept.to_string()).unwrap();
    let fixture = snap["fixtures"][0]["id"].clone();
    send(
        &mut app,
        json!({"cmd": "programmer.select", "fixture": fixture}),
    );
    send(&mut app, json!({"cmd": "group.record", "group": 77}));
    until(&mut app, "the autosave's warning", |m| {
        says(
            m,
            "Kerk.cmdmx.json was changed outside CMDMX: kept as Kerk outside 20",
        )
    });
    let aside = find_soon(&shows, "Kerk outside ");
    assert_eq!(len(&read_soon(&aside, |_| true)["patch"]["fixtures"]), 0);
    let saved = read_soon(&kerk, |v| {
        v["groups"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["id"] == 77)
    });
    assert_eq!(len(&saved["patch"]["fixtures"]), demo_fixtures);
}

#[test]
fn shw_25_report_writes_into_reports_and_answers_the_asker_only() {
    let dir = std::env::temp_dir().join(format!("cmdmx-shw25-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shows = dir.join("Shows");
    let port = free_port();
    let _daemon = Daemon(
        Command::new(env!("CARGO_BIN_EXE_cmdmx-daemon"))
            .env("HOME", &dir)
            .args(["--no-artnet", "--no-sacn", "--no-serial"])
            .args(["--ipc-port", &port.to_string()])
            .arg("--show-dir")
            .arg(&shows)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut app = connect(port);
    assert_eq!(recv(&mut app)["show"]["name"], "Demo");
    let mut other = connect(port);
    assert_eq!(recv(&mut other)["type"], "snapshot");

    // The file lands in <shows folder>/Reports as "Demo yyyy-mm-dd hhmm.html".
    send(&mut app, json!({"cmd": "show.report"}));
    let answer = until(&mut app, "the report", |m| m["type"] == "show.report");
    let path = std::path::PathBuf::from(answer["path"].as_str().unwrap());
    assert!(path.is_absolute(), "{answer}");
    assert_eq!(path.parent().unwrap(), shows.join("Reports"));
    let file = path.file_name().unwrap().to_str().unwrap();
    let minute = file
        .strip_prefix("Demo ")
        .and_then(|f| f.strip_suffix(".html"))
        .unwrap_or_else(|| panic!("{file}"));
    let digits: String = minute.chars().filter(char::is_ascii_digit).collect();
    assert_eq!((minute.len(), digits.len()), (15, 12), "{minute}");
    let html = std::fs::read_to_string(&path).unwrap();
    assert!(html.contains(&format!("<title>Demo — {minute}</title>")));
    for table in ["Fixtures", "Groups", "Palettes", "Playbacks"] {
        assert!(html.contains(&format!("<h2>{table}</h2>")), "{table}");
    }

    // The other client never sees the answer: its own list comes first.
    send(&mut other, json!({"cmd": "show.list"}));
    let first = until(&mut other, "the list", |m| {
        m["type"] == "show.list" || m["type"] == "show.report"
    });
    assert_eq!(first["type"], "show.list");

    // A report changes nothing, so the lock does not refuse it (IPC-50).
    send(
        &mut app,
        json!({"cmd": "lock.set", "on": true, "code": "1234"}),
    );
    until(&mut app, "the lock", |m| {
        m["type"] == "locked" && m["locked"] == true
    });
    send(&mut app, json!({"cmd": "show.report"}));
    let locked = until(&mut app, "the answer", |m| {
        m["type"] == "warning" || m["type"] == "show.report"
    });
    assert_eq!(locked["type"], "show.report", "{locked}");
    send(
        &mut app,
        json!({"cmd": "lock.set", "on": false, "code": "1234"}),
    );
    until(&mut app, "the unlock", |m| {
        m["type"] == "locked" && m["locked"] == false
    });

    // A report that cannot be written is a warning (IPC-06) and the engine runs on.
    std::fs::remove_dir_all(shows.join("Reports")).unwrap();
    std::fs::write(shows.join("Reports"), "not a folder").unwrap();
    send(&mut app, json!({"cmd": "show.report"}));
    let w = until(&mut app, "the warning", |m| {
        m["type"] == "warning" || m["type"] == "show.report"
    });
    assert_eq!(w["type"], "warning", "{w}");
    assert!(
        w["message"]
            .as_str()
            .unwrap()
            .starts_with("Cannot write the report"),
        "{w}"
    );
    send(&mut app, json!({"cmd": "show.list"}));
    until(&mut app, "the list", |m| m["type"] == "show.list");
}

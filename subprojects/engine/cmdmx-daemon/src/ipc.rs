//! IPC v0: WebSocket server, JSON messages and per-tick deltas (IPC-01..07).

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use cmdmx_core::attribute::{AttributeDef, AttributeGroup, AttributeKind};
    use cmdmx_core::engine::{Engine, Settings};
    use cmdmx_core::patch::Fixture;
    use cmdmx_core::playback::{FaderMode, Playback, Priority};
    use cmdmx_core::profile::Profile;
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
            Profile {
                attributes: vec![AttributeDef {
                    name: "Dimmer".into(),
                    group: AttributeGroup::I,
                    kind: AttributeKind::Htp,
                    channel: 1,
                    fine_channel: None,
                }],
            },
        );
        e.patch(Fixture {
            id: 1,
            profile: "dimmer".into(),
            universe: 1,
            address: 1,
        })
        .unwrap();
        e.add_playback(Playback {
            id: 1,
            priority: Priority::Normal,
            fader_mode: FaderMode::Mode0,
            values: BTreeMap::from([(1, BTreeMap::from([("Dimmer".to_string(), 1.0)]))]),
        });
        e
    }

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn ipc_03_snapshot_has_settings_grand_master_and_playbacks() {
        let v = parse(&snapshot(&engine()));
        assert_eq!(
            v,
            json!({"v":0,"type":"snapshot","faders_per_page":8,"grand_master":1.0,
                   "playbacks":[{"id":1,"level":0.0,"active":false}]})
        );
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
        let _ = next_event(&rx);
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

//! ENTTEC Pro serial output node (OUT-09/20/21); threading and health live in `output`.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use cmdmx_protocols::enttec;
use serialport::{
    DataBits, FlowControl, Parity, SerialPort, SerialPortInfo, SerialPortType, StopBits,
};

use crate::output::{Health, OutputState};

const RESCAN: Duration = Duration::from_secs(1);
const WRITE_TIMEOUT: Duration = Duration::from_millis(100);
/// OUT-24 calibration constant, the period of the serial node's own timer (~33 Hz):
/// the Eurolite MK2 restarts its DMX frame when a new USB frame arrives; 40 Hz
/// flickered, <= 33 Hz was steady. `SerialNode::send` also enforces it as a safety net.
pub const MIN_WRITE_INTERVAL: Duration = Duration::from_millis(30);

/// OUT-09: the lowest `/dev/cu.*` path whose USB product string is a known
/// ENTTEC Pro one, with that product string. Anything else is never used.
pub fn pick_port(ports: &[SerialPortInfo]) -> Option<(String, String)> {
    ports
        .iter()
        .filter(|p| p.port_name.starts_with("/dev/cu."))
        .filter_map(|p| match &p.port_type {
            SerialPortType::UsbPort(usb) => usb
                .product
                .as_ref()
                .filter(|name| enttec::is_pro_product_string(name))
                .map(|name| (p.port_name.clone(), name.clone())),
            _ => None,
        })
        .min()
}

/// OUT-23: the once-logged "no interface" line, naming FTDI ports (VID 0x0403)
/// on `/dev/cu.*` that OUT-09 does not use, lowest path first.
pub fn missing_message(ports: &[SerialPortInfo]) -> String {
    let mut ftdi: Vec<(&str, &str)> = ports
        .iter()
        .filter(|p| p.port_name.starts_with("/dev/cu."))
        .filter_map(|p| match &p.port_type {
            SerialPortType::UsbPort(usb) if usb.vid == 0x0403 => {
                Some((p.port_name.as_str(), usb.product.as_deref().unwrap_or("")))
            }
            _ => None,
        })
        .collect();
    ftdi.sort();
    let mut msg = String::from("serial: no ENTTEC Pro interface found");
    for (path, product) in &ftdi {
        msg.push_str("; ");
        msg.push_str(&format!(
            "FTDI \"{product}\" at {path} not used \
             (start with --serial {path} if it is a Eurolite USB-DMX512 PRO MK2)"
        ));
    }
    msg
}

pub struct SerialNode {
    /// `--serial <path>` (OUT-22); `None` = detect (OUT-09).
    fixed_path: Option<String>,
    port: Option<Box<dyn SerialPort>>,
    last_scan: Option<Instant>,
    last_write: Option<Instant>,
    said_missing: bool,
    health: Health,
}

impl SerialNode {
    pub fn new(fixed_path: Option<String>) -> Self {
        SerialNode {
            fixed_path,
            port: None,
            last_scan: None,
            last_write: None,
            said_missing: false,
            health: Health::default(),
        }
    }

    /// Send one frame; returns a log line when something changed. Without a
    /// port it (re)opens at most once per `RESCAN` (OUT-21).
    pub fn send(&mut self, data: &[u8], now: Instant) -> Option<String> {
        if self.port.is_none() {
            if self
                .last_scan
                .is_some_and(|t| now.duration_since(t) < RESCAN)
            {
                return None;
            }
            self.last_scan = Some(now);
            match self.open() {
                Ok(Ok((port, msg))) => {
                    self.port = Some(port);
                    self.last_write = Some(now);
                    let written = self.write(data);
                    return Some(msg)
                        .into_iter()
                        .chain(written)
                        .reduce(|a, b| a + "\n" + &b);
                }
                Ok(Err(missing)) if !self.said_missing && !self.health.is_degraded() => {
                    self.said_missing = true;
                    return Some(missing);
                }
                Ok(Err(_)) => return None,
                Err(e) => return self.log(&Err(e)),
            }
        }
        if self
            .last_write
            .is_some_and(|t| now.duration_since(t) < MIN_WRITE_INTERVAL)
        {
            return None;
        }
        self.last_write = Some(now);
        self.write(data)
    }

    /// IPC-20: no port and not degraded = no interface found (yet).
    pub fn state(&self) -> OutputState {
        match (self.health.state(), &self.port) {
            (OutputState::Ok, None) => OutputState::NoInterface,
            (state, _) => state,
        }
    }

    fn write(&mut self, data: &[u8]) -> Option<String> {
        let result = enttec::send_dmx_frame(data)
            .map_err(io::Error::other)
            .and_then(|frame| match self.port.as_mut() {
                Some(port) => port.write_all(&frame),
                None => Err(io::Error::from(io::ErrorKind::NotConnected)),
            });
        if result.is_err() {
            self.port = None;
        }
        self.log(&result)
    }

    fn log(&mut self, result: &io::Result<()>) -> Option<String> {
        self.health.record(result).map(|m| format!("serial: {m}"))
    }

    /// `Ok(Err(line))` = no interface present, with the OUT-23 log line (not an
    /// error at startup, OUT-21).
    #[allow(clippy::type_complexity)]
    fn open(&self) -> io::Result<Result<(Box<dyn SerialPort>, String), String>> {
        let (path, product) = match &self.fixed_path {
            Some(path) => (path.clone(), "--serial".to_string()),
            None => {
                let ports = serialport::available_ports()?;
                match pick_port(&ports) {
                    Some(found) => found,
                    None => return Ok(Err(missing_message(&ports))),
                }
            }
        };
        let port = serialport::new(&path, enttec::VCP_BAUD)
            .data_bits(DataBits::Eight)
            .parity(Parity::None)
            .stop_bits(StopBits::One)
            .flow_control(FlowControl::None)
            .timeout(WRITE_TIMEOUT)
            .open()?;
        Ok(Ok((
            port,
            format!("serial: ENTTEC Pro \"{product}\" at {path}"),
        )))
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;
    use serialport::{SerialPort, SerialPortInfo, SerialPortType, TTYPort, UsbPortInfo};
    use std::io::Read;
    use std::time::{Duration, Instant};

    fn usb(path: &str, product: Option<&str>) -> SerialPortInfo {
        SerialPortInfo {
            port_name: path.into(),
            port_type: SerialPortType::UsbPort(UsbPortInfo {
                vid: 0x0403,
                pid: 0x6001,
                serial_number: None,
                manufacturer: Some("FTDI".into()),
                product: product.map(String::from),
            }),
        }
    }

    #[test]
    fn out_09_picks_pro_interface_by_product_string_on_cu_path() {
        let ports = [
            SerialPortInfo {
                port_name: "/dev/cu.Bluetooth-Incoming-Port".into(),
                port_type: SerialPortType::Unknown,
            },
            usb("/dev/tty.usbserial-A1", Some("Eurolite DMX512 Pro")),
            usb("/dev/cu.usbserial-B2", Some("FT232R USB UART")),
            usb("/dev/cu.usbserial-A1", Some("Eurolite DMX512 Pro")),
        ];
        let (path, product) = pick_port(&ports).unwrap();
        assert_eq!(path, "/dev/cu.usbserial-A1");
        assert_eq!(product, "Eurolite DMX512 Pro");
    }

    #[test]
    fn out_09_plain_ftdi_is_never_used() {
        let ports = [
            usb("/dev/cu.usbserial-B2", Some("FT232R USB UART")),
            usb("/dev/cu.usbserial-C3", None),
        ];
        assert_eq!(pick_port(&ports), None);
    }

    #[test]
    fn out_09_lowest_path_wins_with_several_matches() {
        let ports = [
            usb("/dev/cu.usbserial-Z9", Some("DMX USB PRO")),
            usb("/dev/cu.usbserial-A1", Some("Eurolite DMX512 Pro")),
        ];
        assert_eq!(pick_port(&ports).unwrap().0, "/dev/cu.usbserial-A1");
    }

    #[test]
    fn out_23_no_interface_line_names_unused_ftdi_ports() {
        let mut other = usb("/dev/cu.usbserial-Z9", Some("Arduino"));
        if let SerialPortType::UsbPort(info) = &mut other.port_type {
            info.vid = 0x2341;
        }
        let ports = [
            usb("/dev/cu.usbserial-B2", None),
            usb("/dev/tty.usbserial-AQ01FA9F", Some("FT232R USB UART")),
            other,
            usb("/dev/cu.usbserial-AQ01FA9F", Some("FT232R USB UART")),
        ];
        assert_eq!(
            missing_message(&ports),
            "serial: no ENTTEC Pro interface found; \
             FTDI \"FT232R USB UART\" at /dev/cu.usbserial-AQ01FA9F not used \
             (start with --serial /dev/cu.usbserial-AQ01FA9F if it is a Eurolite USB-DMX512 PRO MK2); \
             FTDI \"\" at /dev/cu.usbserial-B2 not used \
             (start with --serial /dev/cu.usbserial-B2 if it is a Eurolite USB-DMX512 PRO MK2)"
        );
        assert_eq!(
            missing_message(&ports[2..3]),
            "serial: no ENTTEC Pro interface found"
        );
    }

    #[test]
    fn out_20_frames_arrive_byte_exact_through_the_raw_port() {
        let (mut master, slave) = TTYPort::pair().unwrap();
        master.set_timeout(Duration::from_secs(2)).unwrap();
        // Inject the pty: opening it by path fails on macOS (serialport sets
        // the baud rate with IOSSIOSPEED, which ptys reject). Opening a real
        // device by path is covered by tests/hardware/output.md.
        let mut node = SerialNode::new(None);
        node.port = Some(Box::new(slave));
        // 0x0A would become 0x0D 0x0A on a port that is not raw.
        let mut frame = [0u8; 512];
        frame[0] = 0x0A;
        frame[1] = 0x0D;
        frame[511] = 0x7E;
        assert_eq!(node.send(&frame, Instant::now()), None);
        let expected = cmdmx_protocols::enttec::send_dmx_frame(&frame).unwrap();
        let mut got = vec![0u8; expected.len()];
        master.read_exact(&mut got).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn out_24_at_most_one_write_per_30_ms() {
        let (mut master, slave) = TTYPort::pair().unwrap();
        master.set_timeout(Duration::from_millis(200)).unwrap();
        let mut node = SerialNode::new(None);
        node.port = Some(Box::new(slave));
        let t0 = Instant::now();
        let frame_len = cmdmx_protocols::enttec::send_dmx_frame(&[0; 512])
            .unwrap()
            .len();
        let mut written = Vec::new();
        for ms in [0u64, 25, 50, 75, 80, 100] {
            assert_eq!(
                node.send(&[ms as u8; 512], t0 + Duration::from_millis(ms)),
                None
            );
            let mut got = vec![0u8; frame_len];
            if master.read_exact(&mut got).is_ok() {
                written.push(got[5]);
            }
        }
        // Written at 0, 50 and 80 (30 ms after 50); 25, 75 and 100 come too soon.
        assert_eq!(written, [0, 50, 80]);
    }

    #[test]
    fn ipc_20_serial_state_no_interface_ok_and_degraded() {
        use crate::output::OutputState;
        assert_eq!(SerialNode::new(None).state(), OutputState::NoInterface);
        let (_master, slave) = TTYPort::pair().unwrap();
        let mut node = SerialNode::new(None);
        node.port = Some(Box::new(slave));
        assert_eq!(node.send(&[0; 512], Instant::now()), None);
        assert_eq!(node.state(), OutputState::Ok);
        let mut missing = SerialNode::new(Some("/dev/cu.does-not-exist".into()));
        missing.send(&[0; 512], Instant::now());
        assert_eq!(missing.state(), OutputState::Degraded);
    }

    #[test]
    fn out_21_missing_device_is_degraded_and_rescanned_at_most_once_per_second() {
        let mut node = SerialNode::new(Some("/dev/cu.does-not-exist".into()));
        let t0 = Instant::now();
        assert!(node.send(&[0; 512], t0).unwrap().contains("Degraded"));
        assert_eq!(node.send(&[0; 512], t0 + Duration::from_millis(500)), None);
        assert_eq!(node.last_scan, Some(t0), "no rescan within a second");
        assert_eq!(node.send(&[0; 512], t0 + Duration::from_millis(1100)), None);
        assert_eq!(node.last_scan, Some(t0 + Duration::from_millis(1100)));
    }
}

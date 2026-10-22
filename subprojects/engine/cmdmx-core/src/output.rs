//! Output nodes: where a line (universe) is sent (`specs/modules/output-protocols.md`
//! OUT-13, OUT-25..28). Pure data and checks; the daemon does the I/O.

use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};

use crate::{CoreError, UniverseId};

/// OUT-13: above every node id ever used, so ids are never reused.
pub type NodeId = u32;

/// OUT-13: one destination of a line. Saved flat: `kind` is a tag next to the other keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub line: UniverseId,
    pub enabled: bool,
    #[serde(flatten)]
    pub kind: NodeKind,
}

/// OUT-13: addresses stay strings so the core checks them (OUT-28).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum NodeKind {
    Artnet {
        port_address: u16,
        target: String,
    },
    Sacn {
        universe: u16,
        priority: u8,
        unicast: Option<String>,
    },
    Serial {
        port: String,
    },
}

impl NodeKind {
    /// OUT-25: Art-Net port-address `line - 1`, broadcast.
    pub fn default_artnet(line: UniverseId) -> Self {
        NodeKind::Artnet {
            port_address: line.saturating_sub(1),
            target: "255.255.255.255".into(),
        }
    }

    /// OUT-25: sACN universe `line`, priority 100, multicast.
    pub fn default_sacn(line: UniverseId) -> Self {
        NodeKind::Sacn {
            universe: line,
            priority: 100,
            unicast: None,
        }
    }
}

/// Art-Net's `net:subnet:universe` notation of a port-address (`1` is `0:0:1`).
pub fn artnet_address(port_address: u16) -> String {
    format!(
        "{}:{}:{}",
        port_address >> 8,
        (port_address >> 4) & 15,
        port_address & 15
    )
}

fn invalid(text: String) -> CoreError {
    CoreError::InvalidOutput(text)
}

fn ipv4(what: &str, text: &str) -> Result<(), CoreError> {
    text.parse::<Ipv4Addr>()
        .map(|_| ())
        .map_err(|_| invalid(format!("{what} '{text}' is not an IPv4 address")))
}

/// OUT-28: the fields in range, addresses parse, and (when enabled) no other enabled node
/// (another id) sends to the same destination. Used by every edit and by load.
pub fn check<'a>(node: &Node, others: impl IntoIterator<Item = &'a Node>) -> Result<(), CoreError> {
    if node.line == 0 {
        return Err(invalid(
            "universe 0 does not exist (universes start at 1)".into(),
        ));
    }
    match &node.kind {
        NodeKind::Artnet {
            port_address,
            target,
        } => {
            if *port_address > 32767 {
                return Err(invalid(format!(
                    "Art-Net port-address {port_address} is out of range (0 to 32767)"
                )));
            }
            ipv4("Art-Net target", target)?;
        }
        NodeKind::Sacn {
            universe,
            priority,
            unicast,
        } => {
            if !(1..=63999).contains(universe) {
                return Err(invalid(format!(
                    "sACN universe {universe} is out of range (1 to 63999)"
                )));
            }
            if *priority > 200 {
                return Err(invalid(format!(
                    "sACN priority {priority} is out of range (0 to 200)"
                )));
            }
            if let Some(unicast) = unicast {
                ipv4("sACN unicast address", unicast)?;
            }
        }
        NodeKind::Serial { .. } => {}
    }
    if !node.enabled {
        return Ok(());
    }
    for other in others {
        if other.id == node.id || !other.enabled {
            continue;
        }
        let clash = match (&node.kind, &other.kind) {
            (
                NodeKind::Artnet {
                    port_address: a,
                    target: ta,
                },
                NodeKind::Artnet {
                    port_address: b,
                    target: tb,
                },
            ) if a == b && ta == tb => Some(format!("Art-Net {} at {ta}", artnet_address(*a))),
            (
                NodeKind::Sacn {
                    universe: a,
                    unicast: ua,
                    ..
                },
                NodeKind::Sacn {
                    universe: b,
                    unicast: ub,
                    ..
                },
            ) if a == b && ua == ub => Some(match ua {
                Some(ip) => format!("sACN {a} at {ip}"),
                None => format!("sACN {a} multicast"),
            }),
            (NodeKind::Serial { port: a }, NodeKind::Serial { port: b }) if a == b => {
                Some(format!("serial {a}"))
            }
            _ => None,
        };
        if let Some(what) = clash {
            return Err(invalid(format!(
                "{what} already sends universe {}",
                other.line
            )));
        }
    }
    Ok(())
}

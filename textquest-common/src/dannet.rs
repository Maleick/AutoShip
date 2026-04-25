//! DanNet peer-to-peer cross-client transport types.
use serde::{Deserialize, Serialize};

pub const MAX_PEER_NAME_LEN: usize = 64;
pub const MAX_GROUP_NAME_LEN: usize = 64;
pub const MAX_TLO_EXPR_LEN: usize = 256;
pub const DANNET_PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DanNetPeer {
    pub name: String,
    pub host: String,
    pub port: u16,
}

impl DanNetPeer {
    pub fn validate(&self) -> anyhow::Result<()> {
        use anyhow::bail;
        if self.name.trim().is_empty() { bail!("DanNet peer name must not be empty"); }
        if self.name.len() > MAX_PEER_NAME_LEN { bail!("DanNet peer name exceeds {MAX_PEER_NAME_LEN} chars"); }
        if self.host.trim().is_empty() { bail!("DanNet peer host must not be empty"); }
        if self.port == 0 { bail!("DanNet peer port must be non-zero"); }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TloQuery {
    pub correlation_id: u64,
    pub expression: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TloResponse {
    pub correlation_id: u64,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DanNetCommand {
    Info,
    Peer { peer: String, command: String },
    Group { group: String, command: String },
    AllExecute { command: String },
    GroupGlobalAllExecute { group: String, command: String },
}

impl DanNetCommand {
    #[must_use]
    pub fn to_wire_message(&self, from_peer: &str) -> DanNetMessage {
        match self {
            Self::Peer { peer, command } => DanNetMessage::Execute {
                from: from_peer.to_string(),
                target: DanNetTarget::Peer { name: peer.clone() },
                command: command.clone(),
            },
            Self::Group { group, command } => DanNetMessage::Execute {
                from: from_peer.to_string(),
                target: DanNetTarget::Group { name: group.clone() },
                command: command.clone(),
            },
            Self::AllExecute { command } | Self::GroupGlobalAllExecute { command, .. } => {
                DanNetMessage::Execute {
                    from: from_peer.to_string(),
                    target: DanNetTarget::All,
                    command: command.clone(),
                }
            }
            Self::Info => DanNetMessage::PeerList { peers: Vec::new() },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DanNetTarget {
    Peer { name: String },
    Group { name: String },
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DanNetMessage {
    Hello { peer_name: String, version: u16 },
    Execute { from: String, target: DanNetTarget, command: String },
    TloQuery { from: String, query: TloQuery },
    TloResponse { to: String, response: TloResponse },
    Observe { from: String, expression: String },
    Unobserve { from: String, expression: String },
    ObserveUpdate { from: String, expression: String, value: Option<String> },
    PeerList { peers: Vec<DanNetPeer> },
    Ping,
    Pong,
}

#[must_use]
pub fn parse_dannet_command(input: &str) -> Option<Result<DanNetCommand, String>> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') { return None; }
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let verb = parts.next()?.to_ascii_lowercase();
    let rest = parts.next().unwrap_or("").trim();
    match verb.as_str() {
        "/dnet" => {
            if rest.eq_ignore_ascii_case("info") || rest.is_empty() {
                Some(Ok(DanNetCommand::Info))
            } else {
                Some(Err(format!("unknown /dnet subcommand: {rest}")))
            }
        }
        "/dn" => Some(parse_peer_command(rest)),
        "/dg" => Some(parse_group_command(rest)),
        "/dgae" => {
            if rest.is_empty() {
                Some(Err("/dgae requires a command".to_string()))
            } else {
                Some(Ok(DanNetCommand::AllExecute { command: rest.to_string() }))
            }
        }
        "/dggaexecute" => Some(parse_group_global_all_execute(rest)),
        _ => None,
    }
}

fn parse_peer_command(rest: &str) -> Result<DanNetCommand, String> {
    let mut parts = rest.splitn(2, char::is_whitespace);
    let peer = parts.next().map(str::trim).filter(|s| !s.is_empty())
        .ok_or_else(|| "/dn requires a peer name".to_string())?;
    let command = parts.next().map(str::trim).filter(|s| !s.is_empty())
        .ok_or_else(|| "/dn requires a command after the peer name".to_string())?;
    Ok(DanNetCommand::Peer { peer: peer.to_string(), command: command.to_string() })
}

fn parse_group_command(rest: &str) -> Result<DanNetCommand, String> {
    let mut parts = rest.splitn(2, char::is_whitespace);
    let group = parts.next().map(str::trim).filter(|s| !s.is_empty())
        .ok_or_else(|| "/dg requires a group name".to_string())?;
    let command = parts.next().map(str::trim).filter(|s| !s.is_empty())
        .ok_or_else(|| "/dg requires a command after the group name".to_string())?;
    Ok(DanNetCommand::Group { group: group.to_string(), command: command.to_string() })
}

fn parse_group_global_all_execute(rest: &str) -> Result<DanNetCommand, String> {
    let mut parts = rest.splitn(2, char::is_whitespace);
    let group = parts.next().map(str::trim).filter(|s| !s.is_empty())
        .ok_or_else(|| "/dggaexecute requires a group name".to_string())?;
    let command = parts.next().map(str::trim).filter(|s| !s.is_empty())
        .ok_or_else(|| "/dggaexecute requires a command after the group name".to_string())?;
    Ok(DanNetCommand::GroupGlobalAllExecute { group: group.to_string(), command: command.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dnet_info() {
        assert_eq!(parse_dannet_command("/dnet info"), Some(Ok(DanNetCommand::Info)));
    }

    #[test]
    fn parse_dnet_empty_is_info() {
        assert_eq!(parse_dannet_command("/dnet"), Some(Ok(DanNetCommand::Info)));
    }

    #[test]
    fn parse_dnet_unknown_subcommand_err() {
        assert!(matches!(parse_dannet_command("/dnet peers"), Some(Err(_))));
    }

    #[test]
    fn parse_dn_peer_command() {
        assert_eq!(
            parse_dannet_command("/dn Warrior /assist"),
            Some(Ok(DanNetCommand::Peer { peer: "Warrior".to_string(), command: "/assist".to_string() }))
        );
    }

    #[test]
    fn parse_dn_missing_peer_err() {
        assert!(matches!(parse_dannet_command("/dn"), Some(Err(_))));
    }

    #[test]
    fn parse_dn_missing_command_err() {
        assert!(matches!(parse_dannet_command("/dn Warrior"), Some(Err(_))));
    }

    #[test]
    fn parse_dg_group_command() {
        assert_eq!(
            parse_dannet_command("/dg healers /cast 3"),
            Some(Ok(DanNetCommand::Group { group: "healers".to_string(), command: "/cast 3".to_string() }))
        );
    }

    #[test]
    fn parse_dg_missing_group_err() {
        assert!(matches!(parse_dannet_command("/dg"), Some(Err(_))));
    }

    #[test]
    fn parse_dg_missing_command_err() {
        assert!(matches!(parse_dannet_command("/dg healers"), Some(Err(_))));
    }

    #[test]
    fn parse_dgae_all_execute() {
        assert_eq!(
            parse_dannet_command("/dgae /sit"),
            Some(Ok(DanNetCommand::AllExecute { command: "/sit".to_string() }))
        );
    }

    #[test]
    fn parse_dgae_missing_command_err() {
        assert!(matches!(parse_dannet_command("/dgae"), Some(Err(_))));
    }

    #[test]
    fn parse_dggaexecute() {
        assert_eq!(
            parse_dannet_command("/dggaexecute maintank /assist"),
            Some(Ok(DanNetCommand::GroupGlobalAllExecute {
                group: "maintank".to_string(),
                command: "/assist".to_string(),
            }))
        );
    }

    #[test]
    fn parse_dggaexecute_missing_group_err() {
        assert!(matches!(parse_dannet_command("/dggaexecute"), Some(Err(_))));
    }

    #[test]
    fn parse_dggaexecute_missing_command_err() {
        assert!(matches!(parse_dannet_command("/dggaexecute maintank"), Some(Err(_))));
    }

    #[test]
    fn unrecognized_returns_none() {
        assert!(parse_dannet_command("/bc whatever").is_none());
        assert!(parse_dannet_command("/sit").is_none());
        assert!(parse_dannet_command("not a slash").is_none());
    }

    #[test]
    fn to_wire_peer_execute() {
        let cmd = DanNetCommand::Peer { peer: "Warrior".to_string(), command: "/assist".to_string() };
        assert_eq!(
            cmd.to_wire_message("Cleric01"),
            DanNetMessage::Execute {
                from: "Cleric01".to_string(),
                target: DanNetTarget::Peer { name: "Warrior".to_string() },
                command: "/assist".to_string(),
            }
        );
    }

    #[test]
    fn to_wire_all_execute() {
        let cmd = DanNetCommand::AllExecute { command: "/sit".to_string() };
        assert_eq!(
            cmd.to_wire_message("Cleric01"),
            DanNetMessage::Execute {
                from: "Cleric01".to_string(),
                target: DanNetTarget::All,
                command: "/sit".to_string(),
            }
        );
    }

    #[test]
    fn group_global_all_execute_maps_to_all_target() {
        let cmd = DanNetCommand::GroupGlobalAllExecute {
            group: "maintank".to_string(),
            command: "/assist".to_string(),
        };
        assert_eq!(
            cmd.to_wire_message("Rogue"),
            DanNetMessage::Execute {
                from: "Rogue".to_string(),
                target: DanNetTarget::All,
                command: "/assist".to_string(),
            }
        );
    }

    #[test]
    fn peer_validate_rejects_empty_name() {
        let p = DanNetPeer { name: String::new(), host: "10.0.0.1".to_string(), port: 2114 };
        assert!(p.validate().is_err());
    }

    #[test]
    fn peer_validate_rejects_oversized_name() {
        let p = DanNetPeer { name: "x".repeat(MAX_PEER_NAME_LEN + 1), host: "10.0.0.1".to_string(), port: 2114 };
        assert!(p.validate().is_err());
    }

    #[test]
    fn peer_validate_rejects_zero_port() {
        let p = DanNetPeer { name: "Warrior".to_string(), host: "10.0.0.1".to_string(), port: 0 };
        assert!(p.validate().is_err());
    }

    #[test]
    fn peer_validate_passes() {
        let p = DanNetPeer { name: "Warrior".to_string(), host: "10.0.0.1".to_string(), port: 2114 };
        assert!(p.validate().is_ok());
    }

    #[test]
    fn execute_roundtrips_json() {
        let msg = DanNetMessage::Execute {
            from: "Warrior".to_string(),
            target: DanNetTarget::Group { name: "maintank".to_string() },
            command: "/assist".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: DanNetMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn tlo_query_roundtrips_json() {
        let msg = DanNetMessage::TloQuery {
            from: "Cleric".to_string(),
            query: TloQuery { correlation_id: 42, expression: "Me.HP".to_string() },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: DanNetMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn ping_pong_roundtrip() {
        for msg in [DanNetMessage::Ping, DanNetMessage::Pong] {
            let json = serde_json::to_string(&msg).unwrap();
            let decoded: DanNetMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, decoded);
        }
    }
}

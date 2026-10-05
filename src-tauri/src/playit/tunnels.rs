//! Tunnels (R22, KTD11): one Minecraft Java tunnel per distinct instance port,
//! created lazily on that instance's first launch, and reused afterwards.

use super::api::{AgentTunnel, RunData};

/// The tunnel serving `port`: the one we created (by id) or any existing tunnel on
/// this agent whose local port matches, so tunnels are never duplicated.
pub fn find_for_port<'a>(run: &'a RunData, port: u16, known_id: Option<&str>) -> Option<&'a AgentTunnel> {
    run.tunnels
        .iter()
        .find(|t| known_id == Some(t.id.as_str()))
        .or_else(|| run.tunnels.iter().find(|t| t.local_port() == Some(port)))
}

pub fn is_pending(run: &RunData, id: &str) -> bool {
    run.pending.iter().any(|p| p.id == id)
}

pub fn tunnel_name(port: u16) -> String {
    format!("Lodestar {port}")
}

/// A user-facing message for a tunnel create failure, and whether it means the
/// account's tunnel limit was reached.
pub fn describe_create_failure(reason: &str) -> (bool, String) {
    match reason {
        "RequiresPlayitPremium" | "PublicPortRequiresPlayitPremium" | "RegionRequiresPlayitPremium" | "PortAllocNotFound" => (
            true,
            "Your playit.gg account has no free tunnel left. Remove an old tunnel on playit.gg or give this server the same port as one that already has a tunnel.".into(),
        ),
        "RequiresVerifiedAccount" => (false, "Verify your playit.gg email address, then launch the server again.".into()),
        "AgentVersionTooOld" => (false, "The playit.gg agent is too old for this account.".into()),
        other => (false, format!("playit.gg could not create the tunnel ({other}).")),
    }
}

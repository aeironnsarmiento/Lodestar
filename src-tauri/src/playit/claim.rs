//! One-time account link (R21): generate a claim code, have the user approve it at
//! `https://playit.gg/claim/<code>`, then exchange the code for the agent secret.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{bail, Result};

use super::api::{ApiFailure, ClaimSetup, PlayitApi};

/// Five random bytes as hex, like the official agent.
pub fn generate_code() -> String {
    hex::encode(rand::random::<[u8; 5]>())
}

pub fn claim_url(code: &str) -> String {
    format!("https://playit.gg/claim/{code}")
}

/// Polls until the user approves the claim in their browser, then returns the
/// secret. `poll` is the delay between checks (about a second; gentler than the
/// CLI). `cancel` stops waiting.
pub async fn wait_for_secret(
    api: &PlayitApi,
    code: &str,
    version: &str,
    poll: Duration,
    cancel: &AtomicBool,
    mut on_status: impl FnMut(ClaimSetup),
) -> Result<String> {
    let mut last = None;
    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("Setup was cancelled.");
        }
        match api.claim_setup(code, version).await {
            Ok(Ok(ClaimSetup::UserAccepted)) => break,
            Ok(Ok(ClaimSetup::UserRejected)) => bail!("The link was declined on playit.gg. Start setup again to retry."),
            Ok(Ok(status)) => {
                if last != Some(status) {
                    on_status(status);
                    last = Some(status);
                }
            }
            Ok(Err(ApiFailure::Fail(reason))) if reason == "CodeExpired" => {
                bail!("The setup link expired. Start setup again.")
            }
            Ok(Err(f)) => bail!("playit.gg refused the setup: {f}"),
            // Network trouble: keep waiting, the user may still be approving.
            Err(_) => {}
        }
        tokio::time::sleep(poll).await;
    }

    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("Setup was cancelled.");
        }
        match api.claim_exchange(code).await {
            Ok(Ok(secret)) => return Ok(secret),
            Ok(Err(ApiFailure::Fail(reason))) if reason == "UserRejected" => {
                bail!("The link was declined on playit.gg. Start setup again to retry.")
            }
            Ok(Err(ApiFailure::Fail(reason))) if reason == "CodeExpired" || reason == "CodeNotFound" => {
                bail!("The setup link expired. Start setup again.")
            }
            // NotAccepted / NotSetup: approval is still propagating.
            Ok(Err(ApiFailure::Fail(_))) | Err(_) => {}
            Ok(Err(f)) => bail!("playit.gg refused the setup: {f}"),
        }
        tokio::time::sleep(poll).await;
    }
}

/// `playit.toml` as the agent reads it.
pub fn secret_file_contents(secret: &str) -> String {
    format!("secret_key = \"{}\"\n", secret.trim())
}

/// Reads the secret back from `playit.toml` (or a bare hex key).
pub fn parse_secret_file(text: &str) -> Option<String> {
    let line = text.lines().find_map(|l| {
        let l = l.trim();
        l.strip_prefix("secret_key").map(|rest| rest.trim_start().trim_start_matches('=').trim().trim_matches('"').to_string())
    });
    let secret = line.unwrap_or_else(|| text.trim().to_string());
    (!secret.is_empty() && hex::decode(&secret).is_ok()).then_some(secret)
}

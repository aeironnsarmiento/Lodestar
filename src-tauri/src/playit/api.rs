//! A small client for the playit.gg API, matching the agent's own client (v1.0.10,
//! `packages/api_client`): every call is a JSON POST, authenticated with
//! `Authorization: Agent-Key <secret>`, answering
//! `{"status": "success" | "fail" | "error", "data": ...}`.

use anyhow::{anyhow, bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const DEFAULT_API: &str = "https://api.playit.gg";

/// What a call returned when it did not succeed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiFailure {
    /// An expected failure, e.g. `"NotAccepted"` or `"RequiresPlayitPremium"`.
    Fail(String),
    /// A request-level error (auth, validation, internal).
    Error(String),
}

impl std::fmt::Display for ApiFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiFailure::Fail(s) => write!(f, "{s}"),
            ApiFailure::Error(s) => write!(f, "playit.gg error: {s}"),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "status", content = "data", rename_all = "lowercase")]
enum Envelope {
    Success(Value),
    Fail(Value),
    Error(Value),
}

fn describe(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimSetup {
    WaitingForUserVisit,
    WaitingForUser,
    UserAccepted,
    UserRejected,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RunData {
    pub agent_id: String,
    #[serde(default)]
    pub tunnels: Vec<AgentTunnel>,
    #[serde(default)]
    pub pending: Vec<PendingTunnel>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AgentTunnel {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_address: String,
    #[serde(default)]
    pub tunnel_type: Option<String>,
    #[serde(default)]
    pub agent_config: AgentConfig,
    #[serde(default)]
    pub disabled_reason: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct AgentConfig {
    #[serde(default)]
    pub fields: Vec<ConfigField>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ConfigField {
    pub name: String,
    pub value: String,
}

impl AgentTunnel {
    /// The local port this tunnel forwards to.
    pub fn local_port(&self) -> Option<u16> {
        self.agent_config
            .fields
            .iter()
            .find(|f| f.name == "local_port")
            .and_then(|f| f.value.parse().ok())
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct PendingTunnel {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status_msg: String,
}

#[derive(Clone)]
pub struct PlayitApi {
    client: reqwest::Client,
    base: String,
}

impl PlayitApi {
    pub fn new(client: reqwest::Client, base: impl Into<String>) -> Self {
        Self { client, base: base.into() }
    }

    async fn call<T: DeserializeOwned>(&self, path: &str, secret: Option<&str>, body: Value) -> Result<std::result::Result<T, ApiFailure>> {
        let mut req = self.client.post(format!("{}{path}", self.base)).json(&body);
        if let Some(secret) = secret {
            req = req.header(reqwest::header::AUTHORIZATION, format!("Agent-Key {}", secret.trim()));
        }
        let resp = req.send().await.with_context(|| format!("could not reach playit.gg ({path})"))?;
        if resp.status().as_u16() == 429 {
            bail!("playit.gg is rate limiting requests; try again shortly");
        }
        let text = resp.text().await?;
        let env: Envelope = serde_json::from_str(&text).with_context(|| format!("unexpected playit.gg response for {path}: {text}"))?;
        Ok(match env {
            Envelope::Success(v) => Ok(serde_json::from_value(v).with_context(|| format!("unexpected playit.gg data for {path}"))?),
            Envelope::Fail(v) => Err(ApiFailure::Fail(describe(&v))),
            Envelope::Error(v) => Err(ApiFailure::Error(describe(&v))),
        })
    }

    pub async fn claim_setup(&self, code: &str, version: &str) -> Result<std::result::Result<ClaimSetup, ApiFailure>> {
        self.call("/claim/setup", None, json!({ "code": code, "agent_type": "self-managed", "version": version })).await
    }

    pub async fn claim_exchange(&self, code: &str) -> Result<std::result::Result<String, ApiFailure>> {
        #[derive(Deserialize)]
        struct Key {
            secret_key: String,
        }
        Ok(self.call::<Key>("/claim/exchange", None, json!({ "code": code })).await?.map(|k| k.secret_key))
    }

    pub async fn rundata(&self, secret: &str) -> Result<RunData> {
        self.call("/v1/agents/rundata", Some(secret), json!({}))
            .await?
            .map_err(|f| anyhow!("{f}"))
    }

    /// Creates a Minecraft Java tunnel to `127.0.0.1:<port>` on this agent.
    ///
    /// Uses `/tunnels/create`: the live API rejects the `/v1/tunnels/create` body
    /// published in the agent's client (v1.0.10) with "failed to parse body".
    pub async fn create_tunnel(&self, secret: &str, agent_id: &str, port: u16, name: &str) -> Result<std::result::Result<String, ApiFailure>> {
        #[derive(Deserialize)]
        struct Id {
            id: String,
        }
        let body = json!({
            "name": name,
            "tunnel_type": "minecraft-java",
            "port_type": "tcp",
            "port_count": 1,
            "origin": { "type": "agent", "data": {
                "agent_id": agent_id,
                "local_ip": "127.0.0.1",
                "local_port": port,
            }},
            "enabled": true,
            "alloc": null,
            "firewall_id": null,
            "proxy_protocol": null,
        });
        Ok(self.call::<Id>("/tunnels/create", Some(secret), body).await?.map(|i| i.id))
    }
}

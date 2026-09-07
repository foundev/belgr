//! Cached client for the canonical Agent Client Protocol registry, which
//! lists every ACP agent and how to launch it (npx, uvx, or a platform
//! binary archive).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use futures::StreamExt;
use serde::Deserialize;

use crate::roster::{ExternalAdapter, PendingInstall};

pub const REGISTRY_URL: &str =
    "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";
pub const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const BUNDLED_SNAPSHOT: &str = include_str!("registry_snapshot.json");

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub agents: Vec<Agent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub distribution: Distribution,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Distribution {
    #[serde(default)]
    pub npx: Option<Package>,
    #[serde(default)]
    pub uvx: Option<Package>,
    #[serde(default)]
    pub binary: Option<HashMap<String, BinaryTarget>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Package {
    pub package: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BinaryTarget {
    pub archive: String,
    #[serde(default)]
    pub sha256: String,
    pub cmd: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributionKind {
    Binary,
    Npx,
    Uvx,
}

impl DistributionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::Npx => "npx",
            Self::Uvx => "uvx",
        }
    }
}

impl Agent {
    pub fn preferred_kind(&self, platform: &str) -> Option<DistributionKind> {
        if self
            .distribution
            .binary
            .as_ref()
            .is_some_and(|targets| targets.contains_key(platform))
        {
            Some(DistributionKind::Binary)
        } else if self.distribution.npx.is_some() {
            Some(DistributionKind::Npx)
        } else if self.distribution.uvx.is_some() {
            Some(DistributionKind::Uvx)
        } else {
            None
        }
    }
}

impl Registry {
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).context("parse ACP registry")
    }

    /// Convert every launchable agent into an external adapter. The built-in
    /// Codex and Claude sources are skipped: their adapters already serve
    /// those ids with richer integration. Agents with no distribution for
    /// this platform are skipped too.
    pub fn adapters(&self) -> Vec<ExternalAdapter> {
        let platform = current_platform();
        let mut adapters = Vec::new();
        for agent in &self.agents {
            if matches!(agent.id.as_str(), "codex-acp" | "claude-acp") {
                continue;
            }
            let Some(kind) = agent.preferred_kind(&platform) else {
                continue;
            };
            let Some(adapter) = self.adapter_for(agent, kind, &platform) else {
                continue;
            };
            adapters.push(adapter);
        }
        adapters
    }

    fn adapter_for(
        &self,
        agent: &Agent,
        kind: DistributionKind,
        platform: &str,
    ) -> Option<ExternalAdapter> {
        let version = if agent.version.is_empty() {
            "unversioned"
        } else {
            agent.version.as_str()
        };
        match kind {
            DistributionKind::Npx => {
                let package = agent.distribution.npx.as_ref()?;
                let mut args = vec!["-y".to_string(), package.package.clone()];
                args.extend(package.args.iter().cloned());
                Some(ExternalAdapter {
                    id: agent.id.clone(),
                    label: agent.name.clone(),
                    command: PathBuf::from("npx"),
                    args,
                    env: package.env.clone(),
                    evidence: format!("npx {}@{version}", package.package),
                    platform: false,
                    install: None,
                })
            }
            DistributionKind::Uvx => {
                let package = agent.distribution.uvx.as_ref()?;
                let mut args = vec![package.package.clone()];
                args.extend(package.args.iter().cloned());
                Some(ExternalAdapter {
                    id: agent.id.clone(),
                    label: agent.name.clone(),
                    command: PathBuf::from("uvx"),
                    args,
                    env: package.env.clone(),
                    evidence: format!("uvx {}@{version}", package.package),
                    platform: false,
                    install: None,
                })
            }
            DistributionKind::Binary => {
                let target = agent.distribution.binary.as_ref()?.get(platform)?;
                let install_root = crate::agent_install::default_install_root();
                let cmd = target.cmd.strip_prefix("./").unwrap_or(&target.cmd);
                let installed =
                    crate::agent_install::installed_command(&install_root, &agent.id, version, cmd);
                let (command, install, evidence) = match installed {
                    Some(command) => (command, None, format!("binary {version} (installed)")),
                    None => (
                        install_root.join(&agent.id).join(version).join(cmd),
                        Some(PendingInstall {
                            version: version.to_string(),
                            archive: target.archive.clone(),
                            sha256: target.sha256.clone(),
                            cmd: cmd.to_string(),
                        }),
                        format!("binary {version} (installs on first launch)"),
                    ),
                };
                Some(ExternalAdapter {
                    id: agent.id.clone(),
                    label: agent.name.clone(),
                    command,
                    args: target.args.clone(),
                    env: target.env.clone(),
                    evidence,
                    platform: false,
                    install,
                })
            }
        }
    }
}

/// Registry platform names use `darwin`/`aarch64` spellings.
pub fn current_platform() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "arm64" => "aarch64",
        other => other,
    };
    format!("{os}-{arch}")
}

pub fn default_cache_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("belgr")
        .join("registry-v1.json")
}

/// Fresh cache -> network -> stale cache -> bundled snapshot. Never fails,
/// mirroring the DeepSWE catalog: a registry refresh failure must not block
/// startup, and the bundled snapshot keeps the full agent list present even
/// fully offline.
pub async fn load() -> Registry {
    load_with_cache(&default_cache_path(), CACHE_TTL, REGISTRY_URL).await
}

async fn load_with_cache(cache_path: &Path, ttl: Duration, url: &str) -> Registry {
    load_with_cache_using(cache_path, ttl, || fetch(url)).await
}

async fn load_with_cache_using<F, Fut>(cache_path: &Path, ttl: Duration, fetch: F) -> Registry
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<String>>,
{
    let fresh = cache_path
        .metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age < ttl);
    if fresh
        && let Ok(contents) = std::fs::read_to_string(cache_path)
        && let Ok(registry) = Registry::from_json(&contents)
    {
        return registry;
    }

    match fetch().await {
        Ok(contents) => {
            if let Err(error) = Registry::from_json(&contents) {
                tracing::warn!("fetched ACP registry did not parse: {error:#}");
            } else if let Some(parent) = cache_path.parent() {
                let _ = std::fs::create_dir_all(parent);
                if let Err(error) = std::fs::write(cache_path, &contents) {
                    tracing::warn!(path = %cache_path.display(), "write ACP registry cache: {error}");
                }
            }
            Registry::from_json(&contents)
                .unwrap_or_else(|error| fallback_registry(&format!("{error:#}")))
        }
        Err(fetch_error) => {
            tracing::warn!("refresh ACP registry ({fetch_error:#}); using fallback");
            std::fs::read_to_string(cache_path)
                .ok()
                .and_then(|contents| Registry::from_json(&contents).ok())
                .unwrap_or_else(|| fallback_registry(&format!("{fetch_error:#}")))
        }
    }
}

/// The registry snapshot ships with the binary so the full agent list is
/// available offline. Its parse is validated at build-free test time and in
/// [`Registry::from_json`]; a malformed bundled snapshot still must not take
/// startup down, so the last resort is an empty registry.
fn fallback_registry(reason: &str) -> Registry {
    match Registry::from_json(BUNDLED_SNAPSHOT) {
        Ok(registry) => registry,
        Err(error) => {
            tracing::error!(
                "bundled ACP registry snapshot did not parse: {error} (falling back after {reason})"
            );
            Registry::default()
        }
    }
}

async fn fetch(url: &str) -> Result<String> {
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("mj/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("build ACP registry client")?
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("GET {url}"))?;
    if let Some(length) = response.content_length() {
        anyhow::ensure!(
            length as usize <= MAX_BODY_BYTES,
            "ACP registry body is too large"
        );
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("read ACP registry body")?;
        anyhow::ensure!(
            body.len().saturating_add(chunk.len()) <= MAX_BODY_BYTES,
            "ACP registry body exceeded size limit"
        );
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body).context("ACP registry body is not UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    const FIXTURE: &str = r#"{
        "agents": [{
            "id": "codex-acp", "name": "Codex", "version": "1.0.0",
            "distribution": {
                "binary": {"linux-x86_64": {"archive": "https://example/a.tgz", "sha256": "abcd", "cmd": "./codex-acp"}},
                "npx": {"package": "@agentclientprotocol/codex-acp"}
            }
        }]
    }"#;

    #[test]
    fn parses_registry_and_prefers_platform_binary() {
        let registry = Registry::from_json(FIXTURE).expect("registry");
        assert_eq!(registry.agents[0].name, "Codex");
        assert_eq!(
            registry.agents[0]
                .distribution
                .binary
                .as_ref()
                .and_then(|targets| targets.get("linux-x86_64"))
                .map(|target| target.sha256.as_str()),
            Some("abcd")
        );
        assert_eq!(
            registry.agents[0].preferred_kind("linux-x86_64"),
            Some(DistributionKind::Binary)
        );
        assert_eq!(
            registry.agents[0].preferred_kind("darwin-aarch64"),
            Some(DistributionKind::Npx)
        );
    }

    #[test]
    fn distribution_falls_back_to_uvx_or_none_and_labels_are_stable() {
        let mut agent = Registry::from_json(FIXTURE).unwrap().agents.remove(0);
        agent.distribution.binary = None;
        agent.distribution.npx = None;
        agent.distribution.uvx = Some(Package {
            package: "codex-acp".to_string(),
            args: Vec::new(),
            env: HashMap::new(),
        });
        assert_eq!(
            agent.preferred_kind("linux-x86_64"),
            Some(DistributionKind::Uvx)
        );
        agent.distribution.uvx = None;
        assert_eq!(agent.preferred_kind("linux-x86_64"), None);
        assert_eq!(DistributionKind::Binary.label(), "binary");
        assert_eq!(DistributionKind::Npx.label(), "npx");
        assert_eq!(DistributionKind::Uvx.label(), "uvx");
    }

    #[test]
    fn invalid_registry_json_has_parse_context() {
        let error = Registry::from_json("not json").expect_err("invalid registry");
        assert!(error.to_string().contains("parse ACP registry"), "{error}");
    }

    #[test]
    fn platform_name_uses_registry_conventions() {
        let expected_os = if std::env::consts::OS == "macos" {
            "darwin"
        } else {
            std::env::consts::OS
        };
        let expected_arch = if std::env::consts::ARCH == "arm64" {
            "aarch64"
        } else {
            std::env::consts::ARCH
        };
        assert_eq!(current_platform(), format!("{expected_os}-{expected_arch}"));
    }

    #[tokio::test]
    async fn fresh_valid_cache_skips_fetch() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("registry.json");
        std::fs::write(&cache, FIXTURE).unwrap();
        let called = AtomicBool::new(false);

        let registry = load_with_cache_using(&cache, Duration::from_secs(60), || async {
            called.store(true, Ordering::Relaxed);
            anyhow::bail!("fresh cache unexpectedly fetched")
        })
        .await;

        assert_eq!(registry.agents[0].id, "codex-acp");
        assert!(!called.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn invalid_fresh_cache_is_replaced_by_fetch() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("registry.json");
        std::fs::write(&cache, "not json").unwrap();

        let registry = load_with_cache_using(&cache, Duration::from_secs(60), || async {
            Ok(FIXTURE.to_string())
        })
        .await;

        assert_eq!(registry.agents[0].id, "codex-acp");
        assert_eq!(std::fs::read_to_string(cache).unwrap(), FIXTURE);
    }

    #[tokio::test]
    async fn successful_fetch_creates_cache_parent() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("nested/cache/registry.json");

        let registry =
            load_with_cache_using(&cache, Duration::ZERO, || async { Ok(FIXTURE.to_string()) })
                .await;

        assert_eq!(registry.agents[0].name, "Codex");
        assert_eq!(std::fs::read_to_string(cache).unwrap(), FIXTURE);
    }

    #[tokio::test]
    async fn failed_refresh_falls_back_to_stale_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("registry.json");
        std::fs::write(&cache, FIXTURE).unwrap();

        let registry = load_with_cache_using(&cache, Duration::ZERO, || async {
            anyhow::bail!("network unavailable")
        })
        .await;

        assert_eq!(registry.agents[0].id, "codex-acp");
    }

    #[tokio::test]
    async fn failed_refresh_without_cache_falls_back_to_the_bundled_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("missing.json");

        let registry = load_with_cache_using(&cache, Duration::ZERO, || async {
            anyhow::bail!("network unavailable")
        })
        .await;

        let bundled = Registry::from_json(BUNDLED_SNAPSHOT).expect("bundled snapshot parses");
        assert_eq!(registry.agents.len(), bundled.agents.len());
        assert!(!registry.agents.is_empty());
    }

    #[test]
    fn bundled_snapshot_parses_and_skips_builtins() {
        let bundled = Registry::from_json(BUNDLED_SNAPSHOT).expect("bundled snapshot parses");
        assert!(!bundled.agents.is_empty());
        let adapters = bundled.adapters();
        assert!(!adapters.is_empty());
        assert!(
            adapters
                .iter()
                .all(|adapter| !matches!(adapter.id.as_str(), "codex-acp" | "claude-acp"))
        );
        // Registry agents are opt-in extras, never the platform team.
        assert!(adapters.iter().all(|adapter| !adapter.platform));
    }

    #[test]
    fn npx_agent_maps_to_an_npx_launch() {
        let registry = Registry::from_json(
            r#"{"agents": [{"id": "gemini", "name": "Gemini", "version": "0.58.0",
                "distribution": {"npx": {"package": "@google/gemini-cli", "args": ["--acp"],
                "env": {"NO_AUTOUPDATE": "1"}}}}]}"#,
        )
        .unwrap();
        let adapter = registry.adapters().remove(0);
        assert_eq!(adapter.id, "gemini");
        assert_eq!(adapter.command, PathBuf::from("npx"));
        assert_eq!(adapter.args, vec!["-y", "@google/gemini-cli", "--acp"]);
        assert_eq!(
            adapter.env.get("NO_AUTOUPDATE").map(String::as_str),
            Some("1")
        );
        assert!(adapter.evidence.contains("@google/gemini-cli"));
        assert!(!adapter.platform);
        assert!(adapter.install.is_none());
    }

    #[test]
    fn uvx_agent_maps_to_a_uvx_launch() {
        let registry = Registry::from_json(
            r#"{"agents": [{"id": "fast-agent", "name": "Fast Agent", "version": "0.10.1",
                "distribution": {"uvx": {"package": "fast-agent-acp"}}}]}"#,
        )
        .unwrap();
        let adapter = registry.adapters().remove(0);
        assert_eq!(adapter.command, PathBuf::from("uvx"));
        assert_eq!(adapter.args, vec!["fast-agent-acp"]);
        assert!(adapter.install.is_none());
    }

    #[test]
    fn binary_agent_without_an_install_gets_a_pending_install() {
        let platform = current_platform();
        let registry = Registry::from_json(&format!(
            r#"{{"agents": [{{"id": "opencode", "name": "OpenCode", "version": "1.2.3",
                "distribution": {{"binary": {{"{platform}": {{"archive": "https://example/opencode.tgz",
                "sha256": "abcd", "cmd": "./opencode", "args": ["--stdio"]}}}}}}}}]}}"#
        ))
        .unwrap();
        let adapter = registry.adapters().remove(0);
        let pending = adapter.install.expect("pending install");
        assert_eq!(pending.version, "1.2.3");
        assert_eq!(pending.archive, "https://example/opencode.tgz");
        assert_eq!(pending.cmd, "opencode");
        assert_eq!(adapter.args, vec!["--stdio"]);
        assert!(adapter.evidence.contains("installs on first launch"));
    }

    #[test]
    fn binary_agent_with_a_sentinel_resolves_to_the_installed_command() {
        let platform = current_platform();
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("opencode/1.2.3");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("opencode"), b"binary").unwrap();
        std::fs::write(dir.join(".installed"), "ok").unwrap();

        // Point the install root helper at the fixture dir by overriding the
        // env the agent_install path derives from is not possible; instead
        // assert the mapping falls back to pending when nothing is installed
        // in the real root, and that the sentinel path resolves via
        // agent_install directly (covered there).
        let registry = Registry::from_json(&format!(
            r#"{{"agents": [{{"id": "opencode", "name": "OpenCode", "version": "1.2.3",
                "distribution": {{"binary": {{"{platform}": {{"archive": "https://example/opencode.tgz",
                "sha256": "abcd", "cmd": "./opencode"}}}}}}}}]}}"#
        ))
        .unwrap();
        let adapter = registry.adapters().remove(0);
        assert!(adapter.install.is_some());

        // The installed-command resolver itself honors the sentinel.
        let installed =
            crate::agent_install::installed_command(root.path(), "opencode", "1.2.3", "opencode")
                .expect("installed command");
        assert_eq!(
            installed,
            std::fs::canonicalize(dir.join("opencode")).expect("canonical command")
        );
    }

    #[test]
    fn agent_with_no_distribution_for_this_platform_is_skipped() {
        let other = if current_platform() == "linux-x86_64" {
            "darwin-aarch64"
        } else {
            "linux-x86_64"
        };
        let registry = Registry::from_json(&format!(
            r#"{{"agents": [{{"id": "binary-only", "name": "Binary Only",
                "distribution": {{"binary": {{"{other}": {{"archive": "https://example/a.tgz",
                "sha256": "abcd", "cmd": "./binary-only"}}}}}}}}]}}"#
        ))
        .unwrap();
        assert!(registry.adapters().is_empty());
    }
}

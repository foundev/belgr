//! Anvil adapter registration. Every Anvil-specific fact lives in this
//! crate; `belgr-mj-core` only ever sees a generic external adapter.

use std::collections::HashMap;
use std::path::PathBuf;

use mj_core::roster::ExternalAdapter;

/// The ACP source id Anvil registers and persists under.
pub const SOURCE_ID: &str = "anvil";

/// The platform adapter that launches Anvil. An `MJ_ANVIL_PATH` override
/// pointing at a local binary wins; otherwise the adapter launches the
/// `anvil` binary from `PATH`. A dangling override is honored as-is so it
/// fails loudly at launch instead of being silently replaced.
pub fn adapter() -> ExternalAdapter {
    adapter_from_path(std::env::var_os("MJ_ANVIL_PATH").map(PathBuf::from))
}

fn adapter_from_path(override_path: Option<PathBuf>) -> ExternalAdapter {
    let (command, args, evidence) = match override_path {
        Some(path) => {
            let evidence = format!("MJ_ANVIL_PATH: {}", path.display());
            (path, Vec::new(), evidence)
        }
        None => (
            PathBuf::from("anvil"),
            Vec::new(),
            "anvil (PATH)".to_string(),
        ),
    };
    ExternalAdapter {
        id: SOURCE_ID.to_string(),
        label: "Anvil".to_string(),
        evidence,
        command,
        args,
        env: HashMap::new(),
        platform: true,
        install: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_launch_uses_anvil_from_path() {
        let found = adapter_from_path(None);
        assert_eq!(found.command, PathBuf::from("anvil"));
        assert!(found.args.is_empty());
        assert_eq!(found.id, SOURCE_ID);
        assert_eq!(found.evidence, "anvil (PATH)");
        assert!(found.platform);
        assert!(found.install.is_none());
    }

    #[test]
    fn override_path_replaces_path_lookup() {
        let found = adapter_from_path(Some(PathBuf::from("/opt/anvil/anvil")));
        assert_eq!(found.command, PathBuf::from("/opt/anvil/anvil"));
        assert!(found.args.is_empty());
        assert!(found.evidence.starts_with("MJ_ANVIL_PATH"));
    }

    /// Every test in this binary registers the same set: first-wins global
    /// state stays deterministic no matter which test runs first. Anvil
    /// leads, mirroring a machine where only Anvil is installed.
    fn register_anvil_and_draupnir() {
        mj_core::roster::register_external_adapters(vec![
            adapter_from_path(None),
            draupnir_sibling(),
        ]);
    }

    fn draupnir_sibling() -> ExternalAdapter {
        ExternalAdapter {
            id: "draupnir".to_string(),
            label: "Draupnir".to_string(),
            command: PathBuf::from("draupnir"),
            args: Vec::new(),
            env: HashMap::new(),
            evidence: "draupnir (PATH)".to_string(),
            platform: true,
            install: None,
        }
    }

    #[test]
    fn registered_adapter_becomes_the_implicit_platform_team() {
        register_anvil_and_draupnir();

        let mut config = mj_core::config::Config::default();
        assert!(mj_core::config::has_valid_team(&config));
        assert!(config.apply_registered_external_team());
        assert_eq!(config.agent.acp_source.as_deref(), Some(SOURCE_ID));
        assert_eq!(config.review.acp_source.as_deref(), Some(SOURCE_ID));
        assert_eq!(config.subagents.acp_source.as_deref(), Some(SOURCE_ID));
        assert!(config.agent.discrete_review);

        let inventory = mj_core::roster::discover_inventory(&config);
        let server = inventory
            .servers
            .iter()
            .find(|server| server.id == SOURCE_ID)
            .expect("anvil in inventory");
        assert!(server.selected);
        assert_eq!(inventory.servers[0].id, SOURCE_ID);
    }

    #[test]
    fn disabling_the_preferred_platform_switches_the_team() {
        register_anvil_and_draupnir();

        let mut config = mj_core::config::Config::default();
        assert!(config.apply_registered_external_team());
        assert_eq!(config.agent.acp_source.as_deref(), Some(SOURCE_ID));

        // Disabling Anvil routes the implicit team — and any seat pinned to
        // it — over to Draupnir.
        config.acp.policies.insert(
            SOURCE_ID.to_string(),
            mj_core::config::AcpServerPolicy::Disabled,
        );
        assert!(config.apply_registered_external_team());
        assert_eq!(config.agent.acp_source.as_deref(), Some("draupnir"));
        assert_eq!(config.review.acp_source.as_deref(), Some("draupnir"));
        assert_eq!(config.subagents.acp_source.as_deref(), Some("draupnir"));

        // The disabled platform route no longer claims the inventory.
        let inventory = mj_core::roster::discover_inventory(&config);
        let server = inventory
            .servers
            .iter()
            .find(|server| server.id == SOURCE_ID)
            .expect("anvil in inventory");
        assert!(!server.selected);
    }
}

//! Draupnir adapter registration. Every Draupnir-specific fact lives in this
//! crate; `belgr-mj-core` only ever sees a generic external adapter.

use std::collections::HashMap;
use std::path::PathBuf;

use mj_core::roster::ExternalAdapter;

/// The ACP source id Draupnir registers and persists under.
pub const SOURCE_ID: &str = "draupnir";

/// The platform adapter that launches Draupnir. An `MJ_DRAUPNIR_PATH`
/// override pointing at a local binary wins; otherwise the adapter launches
/// the `draupnir` binary from `PATH`. A dangling override is honored as-is so
/// it fails loudly at launch instead of being silently replaced.
pub fn adapter() -> ExternalAdapter {
    adapter_from_path(std::env::var_os("MJ_DRAUPNIR_PATH").map(PathBuf::from))
}

fn adapter_from_path(override_path: Option<PathBuf>) -> ExternalAdapter {
    let (command, args, evidence) = match override_path {
        Some(path) => {
            let evidence = format!("MJ_DRAUPNIR_PATH: {}", path.display());
            (path, Vec::new(), evidence)
        }
        None => (
            PathBuf::from("draupnir"),
            Vec::new(),
            "draupnir (PATH)".to_string(),
        ),
    };
    ExternalAdapter {
        id: SOURCE_ID.to_string(),
        label: "Draupnir".to_string(),
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
    fn default_launch_uses_draupnir_from_path() {
        let found = adapter_from_path(None);
        assert_eq!(found.command, PathBuf::from("draupnir"));
        assert!(found.args.is_empty());
        assert_eq!(found.id, SOURCE_ID);
        assert_eq!(found.evidence, "draupnir (PATH)");
        assert!(found.platform);
        assert!(found.install.is_none());
    }

    #[test]
    fn override_path_replaces_path_lookup() {
        let found = adapter_from_path(Some(PathBuf::from("/opt/draupnir/draupnir")));
        assert_eq!(found.command, PathBuf::from("/opt/draupnir/draupnir"));
        assert!(found.args.is_empty());
        assert!(found.evidence.starts_with("MJ_DRAUPNIR_PATH"));
    }

    #[test]
    fn registered_adapter_becomes_the_implicit_platform_team() {
        mj_core::roster::register_external_adapters(vec![adapter_from_path(None)]);

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
            .expect("draupnir in inventory");
        assert!(server.selected);
        // Built-in routes may surface too when the host is signed in, but the
        // platform adapter must always lead the inventory.
        assert_eq!(inventory.servers[0].id, SOURCE_ID);
    }
}

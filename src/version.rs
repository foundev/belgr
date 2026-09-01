pub const BELGR_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn belgr_version_label() -> String {
    format!("belgr v{BELGR_VERSION}")
}

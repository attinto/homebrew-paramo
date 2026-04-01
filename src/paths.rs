use std::path::PathBuf;

pub const APP_BINARY_NAME: &str = "paramo";
pub const APP_DISPLAY_NAME: &str = "PARAMO";
pub const APP_SUPPORT_DIR: &str = "/etc/paramo";
pub const SYSTEM_CONFIG_FILE: &str = "/etc/paramo/config.toml";
pub const MANUAL_BINARY_DEST: &str = "/usr/local/bin/paramo";
pub const LOG_FILE: &str = "/var/log/paramo.log";
pub const LAUNCHD_LABEL: &str = "com.paramo.blocker";
pub const PLIST_DEST: &str = "/Library/LaunchDaemons/com.paramo.blocker.plist";

pub const IPC_SOCKET: &str = "/var/run/paramo.sock";

pub const LEGACY_SYSTEM_CONFIG_FILE: &str = "/etc/undistracted/config.toml";
pub const LEGACY_LAUNCHD_LABEL: &str = "com.undistracted.blocker";
pub const LEGACY_PLIST_DEST: &str = "/Library/LaunchDaemons/com.undistracted.blocker.plist";

pub fn system_config_file() -> PathBuf {
    PathBuf::from(SYSTEM_CONFIG_FILE)
}

pub fn system_config_dir() -> PathBuf {
    PathBuf::from(APP_SUPPORT_DIR)
}

pub fn user_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_BINARY_NAME)
}

pub fn user_preferences_file() -> PathBuf {
    user_config_dir().join("preferences.toml")
}

pub fn user_journal_file() -> PathBuf {
    user_config_dir().join("journal.log")
}

pub fn user_witness_file() -> PathBuf {
    user_config_dir().join("witness.log")
}

pub fn brew_bin_candidates() -> [PathBuf; 2] {
    [
        PathBuf::from("/opt/homebrew/bin/paramo"),
        PathBuf::from("/usr/local/bin/paramo"),
    ]
}

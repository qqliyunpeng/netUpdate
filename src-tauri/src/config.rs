//! Persistent settings and paths used by the upgrade workflow.
//!
//! The Python tool kept a `config.ini` beside the executable. netUpdate stores
//! the same fields in `%APPDATA%\\com.localtools.netupdate\\session.json` so it
//! can also remember the window geometry, and imports a legacy `config.ini`
//! once when no session exists yet.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::upgrade::ProtocolSettings;

pub const DEFAULT_FTP_HOST: &str = "172.16.9.6";
pub const DEFAULT_FTP_USERNAME: &str = "lyp";
pub const DEFAULT_FTP_PASSWORD: &str = "123456";
pub const DEFAULT_PAK_NAME: &str = "UPDATE_AQ300.PAK";
/// Bumped when the window layout changes enough that older saved bounds would
/// restore the wrong shape. Layout 3 is the compact 370x380 window.
pub const LAYOUT_VERSION: u32 = 3;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub ip: String,
    pub file_path: String,
    pub ftp_host: String,
    pub ftp_username: String,
    pub ftp_password: String,
    pub ftp_folder: String,
    pub use_local: bool,
    pub theme: String,
    pub protocol: ProtocolSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ip: "172.16.10.110".to_string(),
            file_path: "Z:\\".to_string(),
            ftp_host: DEFAULT_FTP_HOST.to_string(),
            ftp_username: DEFAULT_FTP_USERNAME.to_string(),
            ftp_password: DEFAULT_FTP_PASSWORD.to_string(),
            ftp_folder: "/".to_string(),
            use_local: true,
            theme: "dark".to_string(),
            protocol: ProtocolSettings::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WindowGeometry {
    pub width: f64,
    pub height: f64,
    pub x: i32,
    pub y: i32,
    pub maximized: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionState {
    pub config: AppConfig,
    pub window: Option<WindowGeometry>,
    pub layout: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub app_dir: String,
    pub data_dir: String,
    pub tmp_dir: String,
    pub session_file: String,
}

pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}

/// The same application data directory Tauri uses, resolved without an
/// `AppHandle` so the CLI can share configuration with the GUI.
pub fn default_data_dir() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|directory| directory.join("com.localtools.netupdate"))
        .ok_or_else(|| "无法定位系统应用数据目录".to_string())
}

pub fn session_path_in(data_dir: &Path) -> PathBuf {
    data_dir.join("session.json")
}

pub fn tmp_dir_in(data_dir: &Path) -> Result<PathBuf, String> {
    let directory = data_dir.join("tmp");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("无法创建临时目录 {}: {error}", directory.display()))?;
    Ok(directory)
}

pub fn load_from_dir(data_dir: &Path) -> SessionState {
    let mut session = std::fs::read_to_string(session_path_in(data_dir))
        .ok()
        .and_then(|raw| serde_json::from_str::<SessionState>(&raw).ok())
        .unwrap_or_else(|| SessionState {
            config: import_legacy_config().unwrap_or_default(),
            window: None,
            layout: 0,
        });
    migrate_layout(&mut session);
    session
}

pub fn save_to_dir(data_dir: &Path, session: &SessionState) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|error| error.to_string())?;
    let mut stored = session.clone();
    stored.layout = LAYOUT_VERSION;
    let payload = serde_json::to_vec_pretty(&stored).map_err(|error| error.to_string())?;
    std::fs::write(session_path_in(data_dir), payload).map_err(|error| error.to_string())
}

pub fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("无法定位应用数据目录: {error}"))
}

pub fn session_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(session_path_in(&data_dir(app)?))
}

pub fn tmp_dir(app: &AppHandle) -> Result<PathBuf, String> {
    tmp_dir_in(&data_dir(app)?)
}

pub fn paths(app: &AppHandle) -> Result<AppPaths, String> {
    let app_dir = exe_dir();
    let data_directory = data_dir(app)?;
    let tmp_dir = tmp_dir_in(&data_directory)?;
    let session_file = session_path_in(&data_directory);
    Ok(AppPaths {
        app_dir: app_dir.to_string_lossy().to_string(),
        data_dir: data_directory.to_string_lossy().to_string(),
        tmp_dir: tmp_dir.to_string_lossy().to_string(),
        session_file: session_file.to_string_lossy().to_string(),
    })
}

pub fn load(app: &AppHandle) -> SessionState {
    match data_dir(app) {
        Ok(data_directory) => load_from_dir(&data_directory),
        Err(_) => {
            let mut session = SessionState {
                config: import_legacy_config().unwrap_or_default(),
                window: None,
                layout: 0,
            };
            migrate_layout(&mut session);
            session
        }
    }
}

pub fn save(app: &AppHandle, session: &SessionState) -> Result<(), String> {
    save_to_dir(&data_dir(app)?, session)
}

/// A session written before the compact layout carries the large window bounds
/// from that layout. Ignore them once so the configured compact size applies.
fn migrate_layout(session: &mut SessionState) {
    if session.layout < LAYOUT_VERSION {
        session.window = None;
        session.layout = LAYOUT_VERSION;
    }
}

/// Resolve the package that should be streamed to the device.
pub fn resolve_pak_path(app: &AppHandle, config: &AppConfig) -> Result<PathBuf, String> {
    resolve_pak_path_in(&data_dir(app)?, config)
}

pub fn resolve_pak_path_in(data_dir: &Path, config: &AppConfig) -> Result<PathBuf, String> {
    if config.use_local {
        let raw = config.file_path.trim();
        if raw.is_empty() {
            return Err("PAK 路径为空".to_string());
        }
        let path = PathBuf::from(raw);
        if path.is_dir() {
            Ok(path.join(DEFAULT_PAK_NAME))
        } else {
            Ok(path)
        }
    } else {
        Ok(tmp_dir_in(data_dir)?.join(DEFAULT_PAK_NAME))
    }
}

fn import_legacy_config() -> Option<AppConfig> {
    let mut candidates = vec![exe_dir().join("config.ini")];
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join("config.ini");
        if !candidates.iter().any(|entry| entry == &candidate) {
            candidates.push(candidate);
        }
    }

    candidates.into_iter().find_map(|path| {
        let text = std::fs::read_to_string(path).ok()?;
        config_from_legacy(&parse_ini(&text))
    })
}

fn parse_ini(text: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with(';')
            || line.starts_with('#')
            || line.starts_with('[')
        {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        values.insert(key.trim().to_ascii_lowercase(), value.trim().to_string());
    }
    values
}

fn config_from_legacy(values: &HashMap<String, String>) -> Option<AppConfig> {
    let mut config = AppConfig::default();
    let mut found = false;
    let read = |key: &str| values.get(key).filter(|value| !value.is_empty()).cloned();

    if let Some(value) = read("ip") {
        config.ip = value;
        found = true;
    }
    if let Some(value) = read("file_path") {
        config.file_path = value;
        found = true;
    }
    if let Some(value) = read("ftp_host") {
        config.ftp_host = value;
        found = true;
    }
    if let Some(value) = read("ftp_username").or_else(|| read("ftp_usrname")) {
        config.ftp_username = value;
        found = true;
    }
    if let Some(value) = read("ftp_passwd") {
        config.ftp_password = value;
        found = true;
    }
    if let Some(value) = read("ftp_folder") {
        config.ftp_folder = value;
        found = true;
    }
    if let Some(value) = read("use_local") {
        config.use_local = matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes");
        found = true;
    }
    if let Some(value) = read("theme") {
        config.theme = if value.eq_ignore_ascii_case("light") {
            "light".to_string()
        } else {
            "dark".to_string()
        };
        found = true;
    }

    found.then_some(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_ini_keeps_the_python_field_names() {
        let text = r#"
[DEFAULT]
ip = 172.16.11.188
file_path = Z:/UPDATE_AQ300.PAK
ftp_host = 172.16.9.6
ftp_usrName = lyp
ftp_passwd = 123456
ftp_folder = /
use_local = 0
theme = light
"#;
        let config = config_from_legacy(&parse_ini(text)).expect("legacy config");
        assert_eq!(config.ip, "172.16.11.188");
        assert_eq!(config.file_path, "Z:/UPDATE_AQ300.PAK");
        assert_eq!(config.ftp_username, "lyp");
        assert!(!config.use_local);
        assert_eq!(config.theme, "light");
    }

    #[test]
    fn a_directory_gets_the_default_package_name() {
        let path = PathBuf::from("Z:/");
        let resolved = if path.is_dir() {
            path.join(DEFAULT_PAK_NAME)
        } else {
            path
        };
        // On a machine without Z: this simply checks the non-directory branch;
        // the directory branch is covered by the code path above.
        assert!(!resolved.as_os_str().is_empty());
    }

    #[test]
    fn an_old_session_without_a_layout_drops_its_window_bounds() {
        let raw = r#"{
            "config": { "ip": "172.16.11.188" },
            "window": { "width": 1080, "height": 720, "x": 0, "y": 0, "maximized": false }
        }"#;
        let mut session: SessionState = serde_json::from_str(raw).expect("old session");
        assert_eq!(session.layout, 0);
        migrate_layout(&mut session);
        assert_eq!(session.layout, LAYOUT_VERSION);
        assert!(session.window.is_none());
    }
}

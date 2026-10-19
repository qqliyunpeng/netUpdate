//! netUpdate — AQ300 network upgrade tool.
//!
//! The webview owns only presentation and form state. TCP streaming, FTP,
//! dialogs and persistence live here so the frontend needs no filesystem or
//! network permissions of its own.

mod config;
mod ftp;
mod metrics;
#[cfg(windows)]
mod shortcuts;
mod theme;
mod upgrade;

use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
struct OperationState {
    upgrading: bool,
    downloading: bool,
}

#[tauri::command]
fn load_session(app: AppHandle) -> config::SessionState {
    config::load(&app)
}

#[tauri::command]
fn save_session(app: AppHandle, session: config::SessionState) -> Result<(), String> {
    config::save(&app, &session)
}

#[tauri::command]
fn app_paths(app: AppHandle) -> Result<config::AppPaths, String> {
    config::paths(&app)
}

#[tauri::command]
async fn select_pak_file(app: AppHandle) -> Result<Option<String>, String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("选择 AQ300 升级包")
        .add_filter("PAK 升级包", &["PAK", "pak"])
        .add_filter("所有文件", &["*"])
        .pick_file(move |path| {
            let _ = sender.send(path);
        });

    let selected = receiver
        .recv()
        .map_err(|error| format!("文件对话框失败: {error}"))?;
    match selected {
        Some(path) => {
            let path = path
                .into_path()
                .map_err(|error| format!("无法读取所选路径: {error}"))?;
            Ok(Some(path.to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
}

#[tauri::command]
async fn start_upgrade(
    app: AppHandle,
    state: State<'_, Mutex<OperationState>>,
    config: config::AppConfig,
) -> Result<String, String> {
    {
        let mut operation = state.lock().map_err(|_| "操作状态不可用".to_string())?;
        if operation.upgrading || operation.downloading {
            return Err("已有网络操作正在进行".to_string());
        }
        operation.upgrading = true;
    }

    let result = upgrade::run(app, config).await;
    if let Ok(mut operation) = state.lock() {
        operation.upgrading = false;
    }
    result
}

#[tauri::command]
async fn download_pak(
    app: AppHandle,
    state: State<'_, Mutex<OperationState>>,
    config: config::AppConfig,
) -> Result<String, String> {
    {
        let mut operation = state.lock().map_err(|_| "操作状态不可用".to_string())?;
        if operation.upgrading || operation.downloading {
            return Err("已有网络操作正在进行".to_string());
        }
        operation.downloading = true;
    }

    let result = ftp::download(app, config).await;
    if let Ok(mut operation) = state.lock() {
        operation.downloading = false;
    }
    result
}

#[tauri::command]
fn open_app_directory() -> Result<(), String> {
    let directory = config::exe_dir();
    if !directory.is_dir() {
        return Err(format!("程序目录不存在: {}", directory.display()));
    }

    #[cfg(windows)]
    let result = std::process::Command::new("explorer")
        .arg(&directory)
        .spawn();

    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(&directory).spawn();

    #[cfg(all(not(windows), not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open")
        .arg(&directory)
        .spawn();

    result
        .map(|_| ())
        .map_err(|error| format!("无法打开程序目录 {}: {error}", directory.display()))
}

#[tauri::command]
fn chrome_metrics(window: tauri::WebviewWindow) -> metrics::ChromeMetrics {
    metrics::chrome_metrics(window.scale_factor().unwrap_or(1.0))
}

fn apply_window_geometry(app: &AppHandle) {
    let Some(geometry) = config::load(app).window else {
        return;
    };
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let width = geometry.width.clamp(360.0, 30000.0);
    let height = geometry.height.clamp(370.0, 30000.0);
    resize_client_area(&window, width, height);
    if (-8000..8000).contains(&geometry.x) && (-8000..8000).contains(&geometry.y) {
        let _ = window.set_position(tauri::PhysicalPosition::new(geometry.x, geometry.y));
    }
    if geometry.maximized {
        let _ = window.maximize();
    }
}

/// Restores a client size on an undecorated window.
///
/// `set_size` can land a caption height off on the first call; feeding the
/// measured client size back keeps the window from creeping on each launch.
fn resize_client_area(window: &tauri::WebviewWindow, width: f64, height: f64) {
    let mut target_width = width;
    let mut target_height = height;
    for _ in 0..3 {
        let _ = window.set_size(tauri::PhysicalSize::new(target_width, target_height));
        let Ok(actual) = window.inner_size() else {
            return;
        };
        let offset_x = f64::from(actual.width) - width;
        let offset_y = f64::from(actual.height) - height;
        if offset_x.abs() <= 0.5 && offset_y.abs() <= 0.5 {
            return;
        }
        target_width -= offset_x;
        target_height -= offset_y;
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Mutex::new(OperationState::default()))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            load_session,
            save_session,
            app_paths,
            select_pak_file,
            start_upgrade,
            download_pak,
            open_app_directory,
            chrome_metrics,
            theme::system_caption,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            apply_window_geometry(&handle);
            if let Some(window) = handle.get_webview_window("main") {
                #[cfg(windows)]
                shortcuts::suppress_webview_ctrl_j(&window);
                let _ = window.show();
                let _ = window.set_focus();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

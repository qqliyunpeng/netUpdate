import { invoke } from "@tauri-apps/api/core";
import {
  DEFAULT_CONFIG,
  NO_SYSTEM_CAPTION,
  type AppConfig,
  type AppPaths,
  type ChromeMetrics,
  type ProgressUpdate,
  type SessionState,
  type SystemCaption,
  type WindowGeometry,
} from "./types";

/** True when running inside the Tauri webview (vs. a plain browser tab). */
export const isTauri =
  typeof window !== "undefined" &&
  "__TAURI_INTERNALS__" in (window as unknown as Record<string, unknown>);

const PREVIEW_KEY = "netupdate.preview.session";

export async function loadSession(): Promise<SessionState | null> {
  if (!isTauri) return readPreviewSession();
  return invoke<SessionState | null>("load_session");
}

export async function saveSession(session: SessionState): Promise<void> {
  if (!isTauri) {
    writePreviewSession(session);
    return;
  }
  await invoke("save_session", { session });
}

export async function fetchAppPaths(): Promise<AppPaths> {
  if (!isTauri) {
    return {
      appDir: "浏览器预览",
      dataDir: "localStorage",
      tmpDir: "浏览器预览",
      sessionFile: "localStorage",
    };
  }
  return invoke<AppPaths>("app_paths");
}

export async function selectPakFile(): Promise<string | null> {
  if (!isTauri) return null;
  return invoke<string | null>("select_pak_file");
}

export async function startUpgrade(config: AppConfig): Promise<string> {
  if (!isTauri) throw new Error("浏览器预览模式不能连接升级设备");
  return invoke<string>("start_upgrade", { config });
}

export async function downloadPak(config: AppConfig): Promise<string> {
  if (!isTauri) throw new Error("浏览器预览模式不能连接 FTP 服务器");
  return invoke<string>("download_pak", { config });
}

export async function openAppDirectory(): Promise<void> {
  if (!isTauri) return;
  await invoke("open_app_directory");
}

export async function onUpgradeLog(
  handler: (message: string) => void,
): Promise<() => void> {
  if (!isTauri) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("upgrade-log", (event) => handler(event.payload));
}

export async function onUpgradeProgress(
  handler: (progress: ProgressUpdate) => void,
): Promise<() => void> {
  if (!isTauri) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen<ProgressUpdate>("upgrade-progress", (event) => handler(event.payload));
}

export type WindowAction = "minimize" | "toggle-maximize" | "destroy";

export async function windowAction(action: WindowAction): Promise<void> {
  if (!isTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const win = getCurrentWindow();
  if (action === "minimize") await win.minimize();
  else if (action === "toggle-maximize") await win.toggleMaximize();
  else await win.destroy();
}

export async function isWindowMaximized(): Promise<boolean> {
  if (!isTauri) return false;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow().isMaximized();
}

export async function fetchChromeMetrics(): Promise<ChromeMetrics | null> {
  if (!isTauri) return null;
  try {
    return await invoke<ChromeMetrics>("chrome_metrics");
  } catch {
    return null;
  }
}

export async function fetchSystemCaption(): Promise<SystemCaption> {
  if (!isTauri) return NO_SYSTEM_CAPTION;
  try {
    return await invoke<SystemCaption>("system_caption");
  } catch {
    return NO_SYSTEM_CAPTION;
  }
}

export async function isWindowFocused(): Promise<boolean> {
  if (!isTauri) return document.hasFocus();
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow().isFocused();
}

export async function onWindowFocusChanged(
  handler: (focused: boolean) => void,
): Promise<() => void> {
  if (!isTauri) {
    const focus = () => handler(true);
    const blur = () => handler(false);
    window.addEventListener("focus", focus);
    window.addEventListener("blur", blur);
    return () => {
      window.removeEventListener("focus", focus);
      window.removeEventListener("blur", blur);
    };
  }
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow().onFocusChanged(({ payload }) => handler(payload));
}

export async function windowGeometry(): Promise<WindowGeometry | null> {
  if (!isTauri) return null;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const win = getCurrentWindow();
  const [size, position, maximized] = await Promise.all([
    win.innerSize(),
    win.outerPosition(),
    win.isMaximized(),
  ]);
  return {
    width: size.width,
    height: size.height,
    x: position.x,
    y: position.y,
    maximized,
  };
}

export async function onCloseRequested(
  handler: () => Promise<boolean>,
): Promise<() => void> {
  if (!isTauri) return () => undefined;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const window = getCurrentWindow();
  return window.onCloseRequested(async (event) => {
    event.preventDefault();
    if (await handler()) await window.destroy();
  });
}

function readPreviewSession(): SessionState | null {
  try {
    const raw = localStorage.getItem(PREVIEW_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<SessionState>;
    return {
      config: { ...DEFAULT_CONFIG, ...(parsed.config ?? {}) },
      window: parsed.window ?? null,
    };
  } catch {
    return null;
  }
}

function writePreviewSession(session: SessionState): void {
  try {
    localStorage.setItem(PREVIEW_KEY, JSON.stringify(session));
    localStorage.setItem("netupdate.preview.theme", session.config.theme);
  } catch {
    /* storage disabled — the preview simply does not persist */
  }
}

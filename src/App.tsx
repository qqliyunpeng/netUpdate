import { useCallback, useEffect, useRef, useState } from "react";
import { MenuBar, type MenuDefinition } from "./components/MenuBar";
import { Modal } from "./components/Modal";
import { TabBar, type TabKey } from "./components/TabBar";
import { TitleBar } from "./components/TitleBar";
import { UpgradePage } from "./components/UpgradePage";
import {
  downloadPak,
  fetchAppPaths,
  fetchChromeMetrics,
  isTauri,
  loadSession,
  onCloseRequested,
  onUpgradeLog,
  onUpgradeProgress,
  openAppDirectory,
  saveSession,
  selectPakFile,
  startUpgrade,
  windowAction,
  windowGeometry,
} from "./lib/ipc";
import {
  DEFAULT_CONFIG,
  DEFAULT_PROTOCOL,
  type AppConfig,
  type AppPaths,
  type SessionState,
} from "./lib/types";

const BUILD_TIME = "2026.10.28";
const SAVE_DELAY = 500;
const MAX_LOG_LINES = 1200;

type Busy = "upgrade" | "download" | null;
type ModalState =
  | { kind: "about" }
  | { kind: "alert"; title: string; message: string };

function timestamp(): string {
  const now = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  return `[ ${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}:${pad(
    now.getMinutes(),
  )}:${pad(now.getSeconds())} ]`;
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export default function App() {
  const [ready, setReady] = useState(false);
  const [config, setConfig] = useState<AppConfig>(DEFAULT_CONFIG);
  const [paths, setPaths] = useState<AppPaths | null>(null);
  const [tab, setTab] = useState<TabKey>("network");
  const [busy, setBusy] = useState<Busy>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [ftpOpen, setFtpOpen] = useState(false);
  const [modal, setModal] = useState<ModalState | null>(null);

  const configRef = useRef(config);
  const saveTimer = useRef<number | null>(null);
  const configSeen = useRef(false);
  const startupLogged = useRef(false);

  useEffect(() => {
    configRef.current = config;
  }, [config]);

  useEffect(() => {
    document.documentElement.dataset.theme = config.theme;
  }, [config.theme]);

  useEffect(() => {
    document.title = "netUpdate · AQ300 网络升级";
  }, []);

  const appendLog = useCallback((message: string) => {
    const lines = message
      .split(/\r?\n/)
      .map((line) => line.trimEnd())
      .filter(Boolean)
      .map((line) => `${timestamp()} ${line}`);
    if (!lines.length) return;
    setLogs((previous) => {
      const next = [...previous, ...lines];
      return next.length > MAX_LOG_LINES ? next.slice(-MAX_LOG_LINES) : next;
    });
  }, []);

  const clearLog = useCallback(() => setLogs([]), []);

  const persist = useCallback(async () => {
    const geometry = await windowGeometry().catch(() => null);
    const session: SessionState = { config: configRef.current, window: geometry };
    await saveSession(session).catch(() => undefined);
  }, []);

  const schedulePersist = useCallback(() => {
    if (saveTimer.current !== null) window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      saveTimer.current = null;
      void persist();
    }, SAVE_DELAY);
  }, [persist]);

  useEffect(() => {
    if (!ready) return;
    if (!configSeen.current) {
      configSeen.current = true;
      return;
    }
    schedulePersist();
  }, [config, ready, schedulePersist]);

  useEffect(() => {
    void fetchChromeMetrics().then((chrome) => {
      if (!chrome) return;
      const root = document.documentElement;
      root.dataset.chrome = chrome.measured ? "system" : "platform";
      root.style.setProperty("--titlebar-h", `${chrome.titleHeight}px`);
      root.style.setProperty("--menubar-h", `${chrome.menuHeight}px`);
      root.style.setProperty("--menu-font-size", `${chrome.menuFontSize}px`);
      root.style.setProperty("--tab-h", `${chrome.tabHeight}px`);
    });
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onUpgradeLog((message) => appendLog(message)).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [appendLog]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onUpgradeProgress((update) => setProgress(update.percent)).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let alive = true;
    void (async () => {
      const [session, appPaths] = await Promise.all([
        loadSession().catch(() => null),
        fetchAppPaths().catch(() => null),
      ]);
      if (!alive) return;
      if (session) {
        setConfig({
          ...DEFAULT_CONFIG,
          ...session.config,
          protocol: { ...DEFAULT_PROTOCOL, ...session.config?.protocol },
          theme: session.config?.theme === "light" ? "light" : "dark",
        });
      }
      if (appPaths) setPaths(appPaths);
      setReady(true);
      if (!startupLogged.current) {
        startupLogged.current = true;
        appendLog(`netUpdate v${__APP_VERSION__} 已启动`);
      }
    })();
    return () => {
      alive = false;
    };
  }, [appendLog]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onCloseRequested(async () => {
      await persist();
      return true;
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [persist]);

  const patchConfig = useCallback((patch: Partial<AppConfig>) => {
    setConfig((previous) => ({ ...previous, ...patch }));
  }, []);

  const handleModeChange = useCallback((useLocal: boolean) => {
    setConfig((previous) => ({ ...previous, useLocal }));
  }, []);

  const handleSelectPak = useCallback(async () => {
    try {
      const path = await selectPakFile();
      if (!path) return;
      setConfig((previous) => ({ ...previous, filePath: path, useLocal: true }));
      appendLog(`已选择 PAK: ${path}`);
    } catch (error) {
      const message = errorText(error);
      appendLog(`选择 PAK 失败: ${message}`);
      setModal({ kind: "alert", title: "选择文件失败", message });
    }
  }, [appendLog]);

  const handleUpgrade = useCallback(async () => {
    if (busy !== null) return;
    setBusy("upgrade");
    setProgress(0);
    setTab("network");
    appendLog("开始升级");
    try {
      await startUpgrade(config);
      setProgress(100);
    } catch (error) {
      const message = errorText(error);
      setProgress(null);
      appendLog(`操作失败: ${message}`);
    } finally {
      setBusy(null);
      void persist();
    }
  }, [appendLog, busy, config, persist]);

  const handleDownload = useCallback(async () => {
    if (busy !== null) return;
    setBusy("download");
    setProgress(null);
    setTab("network");
    appendLog("开始从 FTP 下载升级文件");
    try {
      const path = await downloadPak(config);
      appendLog(`FTP 下载完成: ${path}`);
    } catch (error) {
      const message = errorText(error);
      appendLog(`FTP 下载失败: ${message}`);
    } finally {
      setBusy(null);
      void persist();
    }
  }, [appendLog, busy, config, persist]);

  const handleOpenAppDirectory = useCallback(async () => {
    if (!isTauri) {
      setModal({ kind: "alert", title: "浏览器预览", message: "浏览器预览模式不能打开程序目录。" });
      return;
    }
    try {
      await openAppDirectory();
    } catch (error) {
      const message = errorText(error);
      appendLog(`打开程序目录失败: ${message}`);
      setModal({ kind: "alert", title: "打开目录失败", message });
    }
  }, [appendLog]);

  const handleClose = useCallback(async () => {
    await persist();
    await windowAction("destroy");
  }, [persist]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase();
      if (event.altKey && key === "u") {
        event.preventDefault();
        void handleUpgrade();
        return;
      }
      if ((event.ctrlKey || event.metaKey) && key === "l") {
        event.preventDefault();
        clearLog();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [clearLog, handleUpgrade]);

  const canUpgrade =
    isTauri && busy === null && config.ip.trim().length > 0 &&
    (!config.useLocal || config.filePath.trim().length > 0);

  const menus: MenuDefinition[] = [
    {
      label: "升级",
      entries: [
        {
          kind: "action",
          label: "选择本地 PAK…",
          onSelect: () => void handleSelectPak(),
          disabled: busy !== null || !isTauri || !config.useLocal,
        },
        {
          kind: "action",
          label: "PAK 经 FTP 获取",
          onSelect: () => void handleDownload(),
          disabled: busy !== null || !isTauri,
        },
        { kind: "separator" },
        {
          kind: "action",
          label: "开始升级",
          shortcut: "Alt+U",
          onSelect: () => void handleUpgrade(),
          disabled: !canUpgrade,
        },
        { kind: "separator" },
        {
          kind: "action",
          label: "清空日志",
          shortcut: "Ctrl+L",
          onSelect: clearLog,
          disabled: logs.length === 0,
        },
      ],
    },
    {
      label: "视图",
      entries: [
        {
          kind: "action",
          label: "FTP 设置",
          checked: ftpOpen,
          onSelect: () => setFtpOpen((value) => !value),
        },
        { kind: "separator" },
        {
          kind: "action",
          label: "浅色主题",
          checked: config.theme === "light",
          onSelect: () => patchConfig({ theme: "light" }),
        },
        {
          kind: "action",
          label: "深色主题",
          checked: config.theme === "dark",
          onSelect: () => patchConfig({ theme: "dark" }),
        },
      ],
    },
    {
      label: "帮助",
      entries: [
        { kind: "action", label: "打开程序目录", onSelect: () => void handleOpenAppDirectory() },
        { kind: "separator" },
        { kind: "action", label: "关于", onSelect: () => setModal({ kind: "about" }) },
      ],
    },
  ];

  let modalNode = null;
  if (modal?.kind === "about") {
    modalNode = (
      <Modal
        title="关于 netUpdate"
        onDismiss={() => setModal(null)}
        buttons={[{ label: "确定", primary: true, onClick: () => setModal(null) }]}
      >
        <p>{`netUpdate v${__APP_VERSION__}`}</p>
        <p>AQ300 网络升级工具，功能移植自 02.auto_tools。</p>
        <p>Author: lip · Build Time: {BUILD_TIME}</p>
        <p>升级协议、FTP 下载与配置持久化均在 Rust 侧完成。</p>
      </Modal>
    );
  } else if (modal?.kind === "alert") {
    modalNode = (
      <Modal
        title={modal.title}
        onDismiss={() => setModal(null)}
        buttons={[{ label: "确定", primary: true, onClick: () => setModal(null) }]}
      >
        <p className="pre">{modal.message}</p>
      </Modal>
    );
  }

  return (
    <div className="app">
      <TitleBar
        documentName="AQ300 网络升级"
        dirty={busy !== null}
        path={paths?.sessionFile ?? null}
        preview={!isTauri}
        onRequestClose={() => void handleClose()}
      />
      <MenuBar menus={menus} />
      <TabBar active={tab} onChange={setTab} />
      <div className="content">
        {tab === "network" ? (
          <UpgradePage
            config={config}
            paths={paths}
            tauri={isTauri}
            busy={busy}
            ftpOpen={ftpOpen}
            progress={progress}
            logs={logs}
            onConfigChange={patchConfig}
            onModeChange={handleModeChange}
            onToggleFtp={() => setFtpOpen((value) => !value)}
            onSelectPak={() => void handleSelectPak()}
            onUpgrade={() => void handleUpgrade()}
            onDownload={() => void handleDownload()}
            onClearLog={clearLog}
          />
        ) : (
          <section className="workspace single" aria-label="鼠标操作">
            <div className="empty-card">
              <span className="eyebrow">MOUSE OPERATION</span>
              <h1>鼠标操作</h1>
              <p>
                该页签在原版 02.auto_tools 中没有业务控件，此处保留原工具的页签结构。
                网络升级功能全部位于“网络升级”页签。
              </p>
            </div>
          </section>
        )}
      </div>
      {modalNode}
    </div>
  );
}

interface StatusBarProps {
  message: string;
  busy: boolean;
  darkMode: boolean;
  configPath: string;
  onToggleTheme(): void;
}

export function StatusBar({
  message,
  busy,
  darkMode,
  configPath,
  onToggleTheme,
}: StatusBarProps) {
  return (
    <footer className="statusbar">
      <span className="status-dot" data-busy={busy ? "true" : "false"} />
      <span className="status-msg" title={message}>
        {message}
      </span>
      <span className="status-config" title={configPath || "配置保存在应用数据目录"}>
        {configPath || "配置：未加载"}
      </span>
      <button type="button" className="status-theme" onClick={onToggleTheme}>
        {darkMode ? "浅色" : "深色"}
      </button>
    </footer>
  );
}

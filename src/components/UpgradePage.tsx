import { useEffect, useState } from "react";
import { EditInput } from "./EditInput";
import { LogView } from "./LogView";
import { Modal } from "./Modal";
import { ProtocolModal } from "./ProtocolModal";
import { isDefaultProtocol, protocolSummary } from "../lib/protocol";
import { setProtocolWindowExpanded } from "../lib/ipc";
import type { AppConfig, AppPaths } from "../lib/types";

type Busy = "upgrade" | "download" | null;

interface UpgradePageProps {
  config: AppConfig;
  paths: AppPaths | null;
  tauri: boolean;
  busy: Busy;
  ftpOpen: boolean;
  progress: number | null;
  logs: string[];
  onConfigChange(patch: Partial<AppConfig>): void;
  onModeChange(useLocal: boolean): void;
  onToggleFtp(): void;
  onSelectPak(): void;
  onUpgrade(): void;
  onDownload(): void;
  onClearLog(): void;
}

export function UpgradePage({
  config,
  paths,
  tauri,
  busy,
  ftpOpen,
  progress,
  logs,
  onConfigChange,
  onModeChange,
  onToggleFtp,
  onSelectPak,
  onUpgrade,
  onDownload,
  onClearLog,
}: UpgradePageProps) {
  const [protocolOpen, setProtocolOpen] = useState(false);
  const blocked = busy !== null || !tauri;
  const localReady = config.useLocal ? config.filePath.trim().length > 0 : true;
  const canUpgrade = tauri && !blocked && config.ip.trim().length > 0 && localReady;
  const protocolCustom = !isDefaultProtocol(config.protocol);

  useEffect(() => {
    if (!protocolOpen) return;
    void setProtocolWindowExpanded(true);
    return () => {
      void setProtocolWindowExpanded(false);
    };
  }, [protocolOpen]);

  return (
    <section className="workspace compact" aria-label="AQ300 网络升级">
      <LogView
        lines={logs}
        progress={progress}
        busy={busy !== null}
        onClear={onClearLog}
      />

      <div className="config-column compact">
        <div className="compact-row">
          <span className="field-caption">升级方式</span>
          <div className="segmented" role="radiogroup" aria-label="升级方式">
            <button
              type="button"
              role="radio"
              aria-checked={config.useLocal}
              className={config.useLocal ? "on" : ""}
              disabled={blocked}
              title="使用本地 PAK 文件"
              onClick={() => onModeChange(true)}
            >
              本地文件
            </button>
            <button
              type="button"
              role="radio"
              aria-checked={!config.useLocal}
              className={!config.useLocal ? "on" : ""}
              disabled={blocked}
              title="使用 FTP 拉取到 tmp/ 的文件"
              onClick={() => onModeChange(false)}
            >
              FTP 文件
            </button>
          </div>
        </div>

        <div className="compact-row">
          <span className="field-caption">PAK 路径</span>
          <div className="path-row">
            <EditInput
              value={config.filePath}
              placeholder="Z:/ 或完整 PAK 路径"
              title="可填写完整文件路径或目录，目录会自动拼接 UPDATE_AQ300.PAK"
              spellCheck={false}
              autoComplete="off"
              disabled={blocked || !config.useLocal}
              aria-label="PAK 路径"
              onChange={(event) => onConfigChange({ filePath: event.target.value })}
            />
            <button
              type="button"
              className="btn small ghost"
              disabled={blocked || !config.useLocal}
              onClick={onSelectPak}
            >
              选择
            </button>
          </div>
        </div>

        <div className="compact-row">
          <span className="field-caption">主机 IP</span>
          <EditInput
            value={config.ip}
            placeholder="172.16.10.110"
            spellCheck={false}
            autoComplete="off"
            disabled={blocked}
            aria-label="设备 IP"
            onChange={(event) => onConfigChange({ ip: event.target.value })}
          />
        </div>

        <div className="actions compact-actions">
          <button type="button" className="btn small ghost" onClick={onToggleFtp}>
            FTP 设置
          </button>
          <button type="button" className="btn small" disabled={blocked} onClick={onDownload}>
            {busy === "download" ? "下载中…" : "FTP 获取"}
          </button>
          <button
            type="button"
            className="btn small ghost"
            disabled={busy !== null}
            title={`配置升级协议\n当前：${protocolSummary(config.protocol)}`}
            onClick={() => setProtocolOpen(true)}
          >
            <span>协议</span>
            {protocolCustom ? <i className="proto-dot" aria-hidden="true" /> : null}
          </button>
          <button
            type="button"
            className="btn small primary"
            disabled={!canUpgrade}
            onClick={onUpgrade}
          >
            {busy === "upgrade" ? "升级中…" : "升级 (Alt+U)"}
          </button>
        </div>
      </div>

      {ftpOpen ? (
        <Modal
          title="FTP 设置"
          onDismiss={onToggleFtp}
          buttons={[{ label: "完成", primary: true, onClick: onToggleFtp }]}
        >
          <div className="ftp-grid modal-grid">
            <div className="field">
              <span className="field-caption">服务器</span>
              <EditInput
                value={config.ftpHost}
                placeholder="172.16.9.6"
                spellCheck={false}
                autoComplete="off"
                disabled={blocked}
                aria-label="FTP 服务器"
                onChange={(event) => onConfigChange({ ftpHost: event.target.value })}
              />
            </div>
            <div className="field">
              <span className="field-caption">目录</span>
              <EditInput
                value={config.ftpFolder}
                placeholder="/"
                spellCheck={false}
                autoComplete="off"
                disabled={blocked}
                aria-label="FTP 目录"
                onChange={(event) => onConfigChange({ ftpFolder: event.target.value })}
              />
            </div>
            <div className="field">
              <span className="field-caption">用户名</span>
              <EditInput
                value={config.ftpUsername}
                spellCheck={false}
                autoComplete="username"
                disabled={blocked}
                aria-label="FTP 用户名"
                onChange={(event) => onConfigChange({ ftpUsername: event.target.value })}
              />
            </div>
            <div className="field">
              <span className="field-caption">密码</span>
              <EditInput
                type="password"
                value={config.ftpPassword}
                autoComplete="current-password"
                disabled={blocked}
                aria-label="FTP 密码"
                onChange={(event) => onConfigChange({ ftpPassword: event.target.value })}
              />
            </div>
          </div>
          <p className="hint modal-hint">下载目录：{paths?.tmpDir ?? "正在读取…"}</p>
        </Modal>
      ) : null}

      {protocolOpen ? (
        <ProtocolModal
          protocol={config.protocol}
          busy={busy !== null}
          onDismiss={() => setProtocolOpen(false)}
          onSave={(protocol) => {
            onConfigChange({ protocol });
            setProtocolOpen(false);
          }}
        />
      ) : null}
    </section>
  );
}

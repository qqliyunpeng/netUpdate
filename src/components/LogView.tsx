import { useEffect, useRef } from "react";
import { useEditContextMenu } from "./EditContextMenu";

interface LogViewProps {
  lines: string[];
  progress: number | null;
  busy: boolean;
  onClear: () => void;
}

export function LogView({ lines, progress, busy, onClear }: LogViewProps) {
  const logRef = useRef<HTMLTextAreaElement | null>(null);
  const { onContextMenu, menu } = useEditContextMenu({
    readOnly: true,
    extraActions: [{ label: "清空显示", onSelect: onClear }],
  });

  useEffect(() => {
    const element = logRef.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [lines]);

  const percent = progress === null ? 0 : Math.max(0, Math.min(100, progress));

  return (
    <section className="log-card" aria-label="升级日志">
      <div className="log-head">
        <div>
          <h2>运行日志</h2>
          <p>时间戳、连接过程和设备响应都会保留在这里。</p>
        </div>
        <button type="button" className="btn ghost small" onClick={onClear} disabled={!lines.length}>
          清空
        </button>
      </div>
      <textarea
        ref={logRef}
        className="log-text"
        readOnly
        spellCheck={false}
        value={lines.join("\n")}
        placeholder="升级日志会显示在这里。右键可复制、全选或清空。"
        onContextMenu={onContextMenu}
      />
      {menu}
      <div className="progress-track" data-busy={busy ? "true" : "false"}>
        <span className="progress-fill" style={{ width: `${percent}%` }} />
      </div>
      <div className="progress-meta">
        <span>{busy ? (progress === null ? "正在连接…" : `传输进度 ${percent.toFixed(0)}%`) : "空闲"}</span>
        <span>{lines.length ? `共 ${lines.length} 条日志` : "等待操作"}</span>
      </div>
    </section>
  );
}

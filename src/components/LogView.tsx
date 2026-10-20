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
    </section>
  );
}

export type TabKey = "network" | "mouse";

interface TabBarProps {
  active: TabKey;
  onChange(tab: TabKey): void;
}

export function TabBar({ active, onChange }: TabBarProps) {
  return (
    <nav className="tabbar" role="tablist" aria-label="功能页">
      <button
        type="button"
        role="tab"
        aria-selected={active === "network"}
        className={active === "network" ? "tab active" : "tab"}
        onClick={() => onChange("network")}
      >
        网络升级
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={active === "mouse"}
        className={active === "mouse" ? "tab active" : "tab"}
        onClick={() => onChange("mouse")}
      >
        鼠标操作
      </button>
    </nav>
  );
}

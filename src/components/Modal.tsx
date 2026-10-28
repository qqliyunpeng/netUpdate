import { useEffect, useRef, type ReactNode } from "react";

export interface ModalButton {
  label: string;
  primary?: boolean;
  danger?: boolean;
  disabled?: boolean;
  onClick: () => void;
}

interface ModalProps {
  title: string;
  children: ReactNode;
  buttons: ModalButton[];
  onDismiss?: () => void;
  className?: string;
}

export function Modal({ title, children, buttons, onDismiss, className }: ModalProps) {
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!onDismiss) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      const backdrops = document.querySelectorAll<HTMLElement>(".modal-backdrop");
      const topBackdrop = backdrops[backdrops.length - 1];
      if (topBackdrop && dialogRef.current && !topBackdrop.contains(dialogRef.current)) {
        return;
      }
      onDismiss();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onDismiss]);

  return (
    <div
      className="modal-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onDismiss?.();
      }}
    >
      <div
        ref={dialogRef}
        className={className ? `modal ${className}` : "modal"}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <h2 className="modal-title">{title}</h2>
        <div className="modal-body">{children}</div>
        <div className="modal-actions">
          {buttons.map((button) => (
            <button
              type="button"
              key={button.label}
              className={
                button.primary ? "btn primary" : button.danger ? "btn danger" : "btn"
              }
              autoFocus={button.primary && !button.disabled}
              disabled={button.disabled}
              onClick={button.onClick}
            >
              {button.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

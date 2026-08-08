import type { ButtonHTMLAttributes, ReactNode } from "react";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  tone?: "primary" | "quiet" | "danger";
};

export function Button({ tone = "primary", className = "", ...props }: ButtonProps) {
  return <button className={`button button--${tone} ${className}`} {...props} />;
}

type StatePanelProps = {
  eyebrow: string;
  title: string;
  children: ReactNode;
  action?: ReactNode;
  busy?: boolean;
};

export function StatePanel({ eyebrow, title, children, action, busy = false }: StatePanelProps) {
  return (
    <section className="state-panel" aria-live="polite" aria-busy={busy}>
      <span className={`state-panel__mark ${busy ? "state-panel__mark--busy" : ""}`} aria-hidden="true" />
      <div>
        <p className="eyebrow">{eyebrow}</p>
        <h3>{title}</h3>
        <p>{children}</p>
      </div>
      {action && <div className="state-panel__action">{action}</div>}
    </section>
  );
}

export function Confidence({ value }: { value: number }) {
  return (
    <span className="confidence" aria-label={`Confiance ${value} sur 100`}>
      <span aria-hidden="true" style={{ "--confidence": `${value}%` } as React.CSSProperties} />
      {value}% fiable
    </span>
  );
}

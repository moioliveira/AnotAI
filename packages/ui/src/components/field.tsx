import type { ReactNode } from "react";
import { cn } from "../lib/utils";

export interface FieldProps {
  label: string;
  htmlFor?: string;
  hint?: ReactNode;
  children: ReactNode;
  /** Controle à direita do rótulo (switches) em vez de embaixo. */
  inline?: boolean;
  className?: string;
}

export function Field({ label, htmlFor, hint, children, inline, className }: FieldProps) {
  return (
    <div
      className={cn(
        inline ? "flex items-center justify-between gap-6" : "flex flex-col gap-1.5",
        className,
      )}
    >
      <div className="flex flex-col gap-0.5">
        <label htmlFor={htmlFor} className="font-medium text-sm">
          {label}
        </label>
        {hint && <p className="text-muted-foreground text-xs">{hint}</p>}
      </div>
      {children}
    </div>
  );
}

export function Card({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <section className={cn("rounded-lg border border-border bg-card p-5", className)}>
      {children}
    </section>
  );
}

import { clsx } from "clsx";
import type { ReactNode } from "react";

interface StatCardProps {
  label: string;
  value: number;
  icon: ReactNode;
  className?: string;
}

export function StatCard({ label, value, icon, className }: StatCardProps) {
  return (
    <div className={clsx("animate-fade-in group relative overflow-hidden rounded-xl border border-border border-t-2 border-t-primary/20 bg-card p-4 shadow-sm transition-shadow duration-200 hover:shadow-md hover:border-ring/40", className)}>
      <div className="flex items-center justify-between">
        <span className="text-sm text-muted-foreground">{label}</span>
        <span aria-hidden="true" className="rounded-lg bg-muted/50 p-1.5 text-muted-foreground/60 transition-colors group-hover:text-foreground/40">{icon}</span>
      </div>
      <p className="mt-2 text-3xl font-bold tabular-nums tracking-tight">{value}</p>
    </div>
  );
}

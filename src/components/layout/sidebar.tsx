import { NavLink } from "react-router-dom";
import { LayoutDashboard, Package, Shield, Settings, ShoppingBag } from "lucide-react";
import { clsx } from "clsx";

const mainNavItems = [
  { to: "/", icon: LayoutDashboard, label: "Overview" },
  { to: "/extensions", icon: Package, label: "Extensions" },
  { to: "/audit", icon: Shield, label: "Audit" },
  { to: "/marketplace", icon: ShoppingBag, label: "Marketplace" },
];

const utilityNavItems = [
  { to: "/settings", icon: Settings, label: "Settings" },
];

function SidebarLink({ to, icon: Icon, label }: { to: string; icon: React.ElementType; label: string }) {
  return (
    <NavLink
      key={to}
      to={to}
      end={to === "/"}
      className={({ isActive }) =>
        clsx(
          "group relative flex items-center gap-3 rounded-xl px-3 py-2.5 text-[14px] transition-all duration-200 ease-out",
          isActive
            ? "bg-sidebar-accent/90 text-sidebar-accent-foreground font-semibold"
            : "text-sidebar-foreground/60 font-medium hover:bg-sidebar-accent/50 hover:text-sidebar-foreground hover:translate-x-1 hover:scale-[1.01]"
        )
      }
    >
      {({ isActive }) => (
        <>
          <Icon
            size={20}
            strokeWidth={1.75}
            aria-hidden="true"
            className={clsx(
              "transition-colors duration-200",
              isActive && "text-sidebar-primary"
            )}
          />
          {label}
        </>
      )}
    </NavLink>
  );
}

export function Sidebar() {
  return (
    <aside className="flex h-full w-48 shrink-0 flex-col px-3 pb-5 select-none">
      {/* Top spacer for traffic lights */}
      <div className="h-12 shrink-0" />

      <div className="mb-6 px-3">
        <h1 className="text-lg font-bold tracking-tight text-sidebar-foreground">HarnessKit</h1>
        <p className="text-[11px] text-muted-foreground/70">v0.1.0</p>
      </div>

      {/* Branding divider */}
      <div className="mx-3 mb-2 border-t border-sidebar-border/50" />

      <nav className="flex flex-1 flex-col gap-0.5">
        {mainNavItems.map((item) => (
          <SidebarLink key={item.to} {...item} />
        ))}

        {/* Settings separator */}
        <div className="mt-auto mx-3 mb-1 border-t border-sidebar-border/40" />

        {utilityNavItems.map((item) => (
          <SidebarLink key={item.to} {...item} />
        ))}
      </nav>
    </aside>
  );
}

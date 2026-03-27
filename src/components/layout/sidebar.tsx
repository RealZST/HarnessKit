import { NavLink } from "react-router-dom";
import { LayoutDashboard, Package, Shield, Bot, Settings, ShoppingBag } from "lucide-react";
import { clsx } from "clsx";

const navItems = [
  { to: "/", icon: LayoutDashboard, label: "Overview" },
  { to: "/extensions", icon: Package, label: "Extensions" },
  { to: "/marketplace", icon: ShoppingBag, label: "Marketplace" },
  { to: "/audit", icon: Shield, label: "Audit" },
  { to: "/agents", icon: Bot, label: "Agents" },
  { to: "/settings", icon: Settings, label: "Settings" },
];

export function Sidebar() {
  return (
    <aside className="flex h-full w-56 flex-col border-r border-sidebar-border bg-sidebar px-3 py-4">
      <div className="mb-8 px-3">
        <h1 className="text-lg font-bold text-sidebar-foreground">HarnessKit</h1>
        <p className="text-xs text-muted-foreground">v0.1.0</p>
      </div>
      <nav className="flex flex-1 flex-col gap-1">
        {navItems.map(({ to, icon: Icon, label }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              clsx(
                "flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors",
                isActive
                  ? "bg-sidebar-accent text-sidebar-accent-foreground"
                  : "text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-foreground"
              )
            }
          >
            <Icon size={18} />
            {label}
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}

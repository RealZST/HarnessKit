import { clsx } from "clsx";
import {
  Blocks,
  Bot,
  LayoutDashboard,
  Settings,
  Shield,
  ShoppingBag,
} from "lucide-react";
import { NavLink } from "react-router-dom";
import { t } from "@/lib/i18n";
import { isDesktop } from "@/lib/transport";
import { useUIStore } from "@/stores/ui-store";
import { ScopeSwitcher } from "./scope-switcher";
import { UpdateCard } from "./update-card";
import { WebUpdateCard } from "./web-update-card";

const mainNavItems = [
  { to: "/", icon: LayoutDashboard, labelKey: "nav.overview" },
  { to: "/agents", icon: Bot, labelKey: "nav.agents" },
  { to: "/extensions", icon: Blocks, labelKey: "nav.extensions" },
  { to: "/audit", icon: Shield, labelKey: "nav.audit" },
  { to: "/marketplace", icon: ShoppingBag, labelKey: "nav.marketplace" },
];

const utilityNavItems = [
  { to: "/settings", icon: Settings, labelKey: "nav.settings" },
];

function SidebarLink({
  to,
  icon: Icon,
  labelKey,
}: {
  to: string;
  icon: React.ElementType;
  labelKey: string;
}) {
  // Subscribe to language changes so labels re-render
  useUIStore((s) => s.language);
  const label = t(labelKey);
  return (
    <NavLink
      key={to}
      to={to}
      end={to === "/"}
      className={({ isActive }) =>
        clsx(
          "group relative flex items-center gap-3 rounded-xl px-3 py-2.5 text-[14px] font-medium transition-colors duration-150 ease-out",
          isActive
            ? "bg-sidebar-accent/90 text-sidebar-accent-foreground font-semibold"
            : "text-sidebar-foreground/60 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground",
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
              isActive && "text-sidebar-primary",
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
        <h1 className="text-lg font-bold tracking-tight text-sidebar-foreground">
          HarnessKit
        </h1>
      </div>

      {/* Branding divider */}
      <div className="mx-3 mb-2 border-t border-sidebar-border/50" />

      <nav className="flex flex-1 flex-col gap-0.5">
        {mainNavItems.map((item) => (
          <SidebarLink key={item.to} {...item} />
        ))}

        {/* Settings separator */}
        <div className="mt-auto mx-3 mb-1 border-t border-sidebar-border/40" />

        {isDesktop() ? <UpdateCard /> : <WebUpdateCard />}

        <ScopeSwitcher />

        {utilityNavItems.map((item) => (
          <SidebarLink key={item.to} {...item} />
        ))}
      </nav>
    </aside>
  );
}

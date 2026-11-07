import { clsx } from "clsx";
import {
  Blocks,
  Bot,
  FolderGit2,
  LayoutDashboard,
  Package,
  Settings,
  Shield,
  ShoppingBag,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { NavLink } from "react-router-dom";
import { isDesktop } from "@/lib/transport";
import { useServerInfo } from "@/lib/use-server-info";
import { useExtensionStore } from "@/stores/extension-store";
import { ScopeSwitcher } from "./scope-switcher";
import { UpdateCard } from "./update-card";
import { WebUpdateCard } from "./web-update-card";

const mainNavItems = [
  { to: "/", icon: LayoutDashboard, labelKey: "overview" },
  { to: "/agents", icon: Bot, labelKey: "agents" },
  { to: "/extensions", icon: Blocks, labelKey: "extensions" },
  { to: "/kits", icon: Package, labelKey: "kits" },
  { to: "/repositories", icon: FolderGit2, labelKey: "repositories" },
  { to: "/marketplace", icon: ShoppingBag, labelKey: "marketplace" },
  { to: "/audit", icon: Shield, labelKey: "audit" },
] as const;

const utilityNavItems = [
  { to: "/settings", icon: Settings, labelKey: "settings" },
] as const;

function SidebarLink({
  to,
  icon: Icon,
  label,
  badge,
}: {
  to: string;
  icon: React.ElementType;
  label: string;
  badge?: React.ReactNode;
}) {
  return (
    <NavLink
      key={to}
      to={to}
      title={label}
      end={to === "/"}
      className={({ isActive }) =>
        clsx(
          "group relative flex items-center gap-2.5 rounded-xl px-2.5 py-2.5 text-[14px] font-medium transition-colors duration-150 ease-out",
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
          <span className="flex-1 truncate" title={label}>
            {label}
          </span>
          {badge}
        </>
      )}
    </NavLink>
  );
}

export function Sidebar() {
  const { t } = useTranslation("navigation");
  const serverInfo = useServerInfo();
  const newRepoSkillsCount = useExtensionStore((s) => s.newRepoSkills.length);
  return (
    <aside className="flex h-full w-52 shrink-0 flex-col px-3 pb-5 select-none">
      {/* Top spacer for traffic lights */}
      <div className="h-12 shrink-0" />

      <div className="mb-6 px-3">
        <h1 className="text-lg font-bold tracking-tight text-sidebar-foreground">
          HarnessKit
        </h1>
        {serverInfo?.nodeName && (
          <p
            className="mt-0.5 truncate text-xs text-sidebar-foreground/50"
            title={serverInfo.nodeName}
          >
            {serverInfo.nodeName}
          </p>
        )}
      </div>

      {/* Branding divider */}
      <div className="mx-3 mb-2 border-t border-sidebar-border/50" />

      <nav className="flex flex-1 flex-col gap-0.5">
        {mainNavItems.map((item) => (
          <SidebarLink
            key={item.to}
            to={item.to}
            icon={item.icon}
            label={t(item.labelKey)}
            badge={
              item.to === "/extensions" && newRepoSkillsCount > 0 ? (
                <span
                  title={t("newSkillsBadgeTooltip", {
                    count: newRepoSkillsCount,
                  })}
                  className="inline-flex shrink-0 items-center justify-center rounded-full bg-primary/15 px-1.5 py-0.5 text-[10px] font-semibold text-primary"
                >
                  +{newRepoSkillsCount}
                </span>
              ) : undefined
            }
          />
        ))}

        {/* Settings separator */}
        <div className="mt-auto mx-3 mb-1 border-t border-sidebar-border/40" />

        {isDesktop() ? <UpdateCard /> : <WebUpdateCard />}

        <ScopeSwitcher />

        {utilityNavItems.map((item) => (
          <SidebarLink
            key={item.to}
            to={item.to}
            icon={item.icon}
            label={t(item.labelKey)}
          />
        ))}
      </nav>
    </aside>
  );
}

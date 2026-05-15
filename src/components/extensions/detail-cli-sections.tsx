import { useTranslation } from "react-i18next";
import type { Extension, ExtensionKind, GroupedExtension } from "@/lib/types";
import { extensionGroupKey } from "@/lib/types";
import { findCliChildren } from "@/stores/extension-helpers";
import { useExtensionStore } from "@/stores/extension-store";

interface CliSectionsProps {
  group: GroupedExtension;
  extensions: Extension[];
}

export function CliSections({ group, extensions }: CliSectionsProps) {
  const { t } = useTranslation("extensions");
  const setSelectedId = useExtensionStore((s) => s.setSelectedId);
  const grouped = useExtensionStore((s) => s.grouped);

  if (group.kind !== "cli") return null;

  const children = findCliChildren(
    extensions,
    group.instances[0]?.id,
    group.pack,
  );

  // Deduplicate children by groupKey so each child skill/MCP appears once
  const allGroups = grouped();
  const childGroups = new Map<
    string,
    { name: string; kind: ExtensionKind; groupKey: string }
  >();
  for (const child of children) {
    const key = extensionGroupKey(child);
    if (!childGroups.has(key)) {
      const exists = allGroups.some((g) => g.groupKey === key);
      if (exists) {
        childGroups.set(key, {
          name: child.name,
          kind: child.kind,
          groupKey: key,
        });
      }
    }
  }

  return (
    <>
      {/* CLI Details */}
      {group.instances[0]?.cli_meta &&
        (() => {
          const cli_meta = group.instances[0].cli_meta;
          return (
            <div className="mt-4 space-y-3 text-sm">
              <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                {t("cli.title")}
              </h4>
              <div className="grid grid-cols-2 gap-2 text-muted-foreground">
                <span>{t("cli.binary")}</span>
                <span className="font-mono">{cli_meta.binary_name}</span>
                {cli_meta.version && (
                  <>
                    <span>{t("cli.version")}</span>
                    <span>{cli_meta.version}</span>
                  </>
                )}
                {cli_meta.install_method && (
                  <>
                    <span>{t("cli.installedVia")}</span>
                    <span>{cli_meta.install_method}</span>
                  </>
                )}
                {cli_meta.binary_path && (
                  <>
                    <span>{t("cli.path")}</span>
                    <span className="font-mono text-xs break-all">
                      {cli_meta.binary_path}
                    </span>
                  </>
                )}
                {cli_meta.credentials_path && (
                  <>
                    <span>{t("cli.credentials")}</span>
                    <span className="font-mono text-xs break-all">
                      {cli_meta.credentials_path}
                    </span>
                  </>
                )}
              </div>
              {cli_meta.api_domains.length > 0 && (
                <div>
                  <span className="text-muted-foreground">
                    {t("cli.apiDomains")}
                  </span>
                  <div className="flex flex-wrap gap-1 mt-1">
                    {cli_meta.api_domains.map((d) => (
                      <span
                        key={d}
                        className="text-xs px-2 py-0.5 bg-muted rounded-full"
                      >
                        {d}
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </div>
          );
        })()}

      {/* Associated Extensions — grouped by kind in cards */}
      {childGroups.size > 0 &&
        (() => {
          const byKind = new Map<
            ExtensionKind,
            { name: string; kind: ExtensionKind; groupKey: string }[]
          >();
          for (const child of childGroups.values()) {
            const list = byKind.get(child.kind) ?? [];
            list.push(child);
            byKind.set(child.kind, list);
          }
          const kindLabel: Record<string, string> = {
            skill: t("cli.skills"),
            mcp: t("cli.mcpServers"),
            plugin: t("cli.plugins"),
            hook: t("cli.hooks"),
          };
          return (
            <div className="mt-4">
              <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
                {t("cli.associated")}
              </h4>
              <div className="space-y-2">
                {[...byKind.entries()].map(([kind, items]) => (
                  <div
                    key={kind}
                    className="rounded-lg border border-border bg-card p-3"
                  >
                    <span className="text-xs font-medium text-muted-foreground">
                      {t("cli.kindCount", {
                        kind: kindLabel[kind] ?? kind,
                        count: items.length,
                      })}
                    </span>
                    <div className="mt-2 flex flex-wrap gap-1">
                      {items.map((child) => (
                        <button
                          key={child.groupKey}
                          onClick={() => setSelectedId(child.groupKey)}
                          className="rounded-md bg-muted/50 px-2 py-1 text-xs text-foreground hover:bg-accent transition-colors"
                        >
                          {child.name}
                        </button>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          );
        })()}
    </>
  );
}

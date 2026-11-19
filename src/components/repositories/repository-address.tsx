import { Copy, ExternalLink, FolderOpen } from "lucide-react";
import { useTranslation } from "react-i18next";
import { isDesktop, isMac } from "@/lib/transport";
import type { RepositoryWithItems } from "@/lib/types";
import { useAgentConfigStore } from "@/stores/agent-config-store";

/** The end of an address is its most telling part, so a long one is cut in
 *  the middle and keeps this many closing characters. */
const KEPT_END = 16;

const CLASS_NAME =
  "inline-flex min-w-0 max-w-[min(18rem,100%)] items-center gap-1 transition-colors hover:text-foreground";

/** Where a repository lives. A remote one opens its web page. A local one is
 *  revealed in the file manager, or in web mode, where the files are on the
 *  server's machine, has its path copied. */
export function RepositoryAddress({ repo }: { repo: RepositoryWithItems }) {
  const { t } = useTranslation("agents");
  const revealInFinder = useAgentConfigStore((s) => s.revealInFinder);
  const copyPath = useAgentConfigStore((s) => s.copyPath);

  if (!repo.url.startsWith("file://")) {
    const https = repo.url.startsWith("https://");
    // An SSH or git:// URL has no page of its own; its host and path do.
    const text = https ? repo.url.slice("https://".length) : repo.key;
    return (
      <a
        href={https ? repo.url : `https://${repo.key}`}
        target="_blank"
        rel="noopener noreferrer"
        title={repo.url}
        className={CLASS_NAME}
      >
        <MiddleEllipsis text={text} />
        <ExternalLink size={10} className="shrink-0" />
      </a>
    );
  }

  // `file:///C:/skills` on Windows, `file:///home/me/skills` elsewhere.
  const path = repo.url.slice("file://".length).replace(/^\/([A-Za-z]:)/, "$1");
  const desktop = isDesktop();
  const Icon = desktop ? FolderOpen : Copy;
  return (
    <button
      type="button"
      onClick={() => (desktop ? revealInFinder(path) : copyPath(path))}
      title={
        desktop
          ? t(isMac() ? "file.revealInFinder" : "file.revealInExplorer")
          : t("file.copyPath")
      }
      className={CLASS_NAME}
    >
      <MiddleEllipsis text={path} />
      <Icon size={10} className="shrink-0" />
    </button>
  );
}

function MiddleEllipsis({ text }: { text: string }) {
  return (
    <span className="flex min-w-0">
      <span className="truncate">{text.slice(0, -KEPT_END)}</span>
      <span className="shrink-0 whitespace-pre">{text.slice(-KEPT_END)}</span>
    </span>
  );
}

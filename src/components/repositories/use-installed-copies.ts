import { useScope } from "@/hooks/use-scope";
import type { InstalledCopy, RepositoryItem } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { enabledAgentSet, inScope } from "@/stores/extension-helpers";

/** The copies of a repository item that count as installed in this view: in
 *  the scope being viewed and on an agent that is switched on, like every
 *  other list under the scope switcher. */
export function useInstalledCopies(): (
  item: RepositoryItem,
) => InstalledCopy[] {
  const { scope } = useScope();
  const enabled = enabledAgentSet(useAgentStore((s) => s.agents));
  return (item) =>
    inScope(item.installed, scope).filter(
      (copy) => !enabled || enabled.has(copy.agent),
    );
}

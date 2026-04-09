import { ArrowDownCircle, Loader2, Shield, X } from "lucide-react";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { KindBadge } from "@/components/shared/kind-badge";
import { TrustBadge } from "@/components/shared/trust-badge";
import type { GroupedExtension, UpdateStatus } from "@/lib/types";
import { toast } from "@/stores/toast-store";

interface DetailHeaderProps {
  group: GroupedExtension;
  updateStatuses: Map<string, UpdateStatus>;
  updateExtension: (id: string) => Promise<void>;
  onClose: () => void;
}

export function DetailHeader({
  group,
  updateStatuses,
  updateExtension,
  onClose,
}: DetailHeaderProps) {
  const navigate = useNavigate();
  const [updating, setUpdating] = useState(false);

  return (
    <div className="shrink-0 flex items-start justify-between border-b border-border px-5 py-4">
      <div>
        <h3 className="text-lg font-semibold">
          {group.kind === "hook"
            ? (() => {
                const parts = group.name.split(":");
                if (parts.length >= 3) {
                  const command = parts.slice(2).join(":");
                  return command.split(" ").map((t) => t.split("/").pop() || t).join(" ");
                }
                return group.name;
              })()
            : group.name}
        </h3>
        <div className="mt-1 flex items-center gap-2">
          <KindBadge kind={group.kind} />
          {group.trust_score != null && (
            <TrustBadge score={group.trust_score} size="sm" />
          )}
          {group.trust_score != null && (
            <button
              onClick={() => navigate(`/audit?ext=${group.instances[0].id}`)}
              className="flex items-center gap-1 rounded-md px-2 py-0.5 text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
              title="View audit details"
            >
              <Shield size={12} />
              View Audit
            </button>
          )}
          {(() => {
            const hasUpdate = group.instances.some(
              (inst) =>
                updateStatuses.get(inst.id)?.status === "update_available",
            );
            if (!hasUpdate) return null;
            const handleUpdate = async () => {
              setUpdating(true);
              try {
                const inst = group.instances.find(
                  (i) =>
                    updateStatuses.get(i.id)?.status === "update_available",
                );
                if (inst) {
                  await updateExtension(inst.id);
                  toast.success(`${group.name} updated`);
                }
              } catch (e) {
                toast.error(`Update failed: ${e}`);
              } finally {
                setUpdating(false);
              }
            };
            return (
              <button
                onClick={handleUpdate}
                disabled={updating}
                className="flex items-center gap-1 rounded-md bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary hover:bg-primary/20 transition-colors disabled:opacity-50"
              >
                {updating ? (
                  <Loader2 size={12} className="animate-spin" />
                ) : (
                  <ArrowDownCircle size={12} />
                )}
                {updating ? "Updating..." : "Update"}
              </button>
            );
          })()}
        </div>
      </div>
      <button
        onClick={onClose}
        aria-label="Close extension details"
        className="shrink-0 rounded-lg p-2.5 text-muted-foreground hover:text-foreground"
      >
        <X size={18} />
      </button>
    </div>
  );
}

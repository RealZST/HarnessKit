import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { restrictToVerticalAxis } from "@dnd-kit/modifiers";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { clsx } from "clsx";
import { GripVertical } from "lucide-react";
import { useCallback, useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { AgentMascot } from "@/components/shared/agent-mascot/agent-mascot";
import type { AgentDetail } from "@/lib/types";
import { agentDisplayName } from "@/lib/types";
import { useAgentConfigStore } from "@/stores/agent-config-store";
import { useAgentStore } from "@/stores/agent-store";

function SortableAgentItem({
  agent,
  isSelected,
  onSelect,
}: {
  agent: AgentDetail;
  isSelected: boolean;
  onSelect: () => void;
}) {
  const { t } = useTranslation("agents");
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: agent.name });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={clsx(
        "flex items-center rounded-lg transition-colors",
        isDragging && "opacity-50 z-10",
        isSelected
          ? "bg-accent text-accent-foreground"
          : agent.detected
            ? "text-foreground/80 hover:bg-accent/50"
            : "text-muted-foreground/50",
      )}
    >
      <div
        className="flex items-center justify-center w-6 shrink-0 cursor-grab active:cursor-grabbing text-muted-foreground/30 hover:text-muted-foreground/60"
        {...attributes}
        {...listeners}
      >
        <GripVertical size={14} />
      </div>
      <button
        onClick={onSelect}
        disabled={!agent.detected}
        className="flex items-center gap-2 flex-1 py-2.5 pr-3 text-left"
      >
        <AgentMascot name={agent.name} size={18} />
        <div className="min-w-0">
          <span className="block text-[13px] font-medium">
            {agentDisplayName(agent.name)}
          </span>
          {!agent.detected && (
            <span className="block text-[10px] text-muted-foreground leading-tight">
              {t("list.notDetected")}
            </span>
          )}
        </div>
      </button>
    </div>
  );
}

export function AgentList() {
  const { t } = useTranslation("agents");
  const agentDetails = useAgentConfigStore((s) => s.agentDetails);
  const selectedAgent = useAgentConfigStore((s) => s.selectedAgent);
  const selectAgent = useAgentConfigStore((s) => s.selectAgent);
  const agentOrder = useAgentStore((s) => s.agentOrder);
  const reorderAgents = useAgentStore((s) => s.reorderAgents);
  const agents = useAgentStore((s) => s.agents);

  const sorted = useMemo(
    () =>
      [...agentDetails].sort((a, b) => {
        const ai = agentOrder.indexOf(a.name);
        const bi = agentOrder.indexOf(b.name);
        return (ai === -1 ? 99 : ai) - (bi === -1 ? 99 : bi);
      }),
    [agentDetails, agentOrder],
  );

  // A disabled agent (including ones "Detected only" auto-disabled) is hidden
  // from the sidebar. AgentDetail doesn't carry enabled state, so cross-
  // reference the agent store; default to visible when an agent is unknown
  // (store not loaded yet) so the list never flashes empty.
  const disabledNames = useMemo(
    () => new Set(agents.filter((a) => !a.enabled).map((a) => a.name)),
    [agents],
  );

  const visible = useMemo(
    () => sorted.filter((a) => !disabledNames.has(a.name)),
    [sorted, disabledNames],
  );

  const hidden = useMemo(
    () =>
      new Set(
        sorted.filter((a) => disabledNames.has(a.name)).map((a) => a.name),
      ),
    [sorted, disabledNames],
  );

  // Keep the selection on a visible agent — if the selected one gets hidden
  // (disabled), fall back to the first visible, or clear it.
  useEffect(() => {
    if (!selectedAgent) return;
    const isSelectedVisible = visible.some((a) => a.name === selectedAgent);
    if (!isSelectedVisible) {
      selectAgent(visible.length > 0 ? visible[0].name : null);
    }
  }, [visible, selectedAgent, selectAgent]);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );

  const handleDragEnd = useCallback(
    (event: DragEndEvent) => {
      const { active, over } = event;
      if (!over || active.id === over.id) return;

      const visibleNames = visible.map((a) => a.name);
      const oldIndex = visibleNames.indexOf(active.id as string);
      const newIndex = visibleNames.indexOf(over.id as string);
      if (oldIndex === -1 || newIndex === -1) return;

      const reorderedVisible = arrayMove(visibleNames, oldIndex, newIndex);

      let fullOrder: string[];
      if (hidden.size === 0) {
        fullOrder = reorderedVisible;
      } else {
        const result: string[] = [];
        let vi = 0;
        for (const name of agentOrder) {
          if (hidden.has(name)) {
            result.push(name);
          } else {
            if (vi < reorderedVisible.length) {
              result.push(reorderedVisible[vi]);
              vi++;
            }
          }
        }
        while (vi < reorderedVisible.length) {
          result.push(reorderedVisible[vi]);
          vi++;
        }
        fullOrder = result;
      }

      reorderAgents(fullOrder);
    },
    [visible, hidden, agentOrder, reorderAgents],
  );

  if (visible.length === 0) {
    return (
      <div className="flex flex-col gap-0.5 p-2">
        <div className="px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          {t("list.header")}
        </div>
        <div className="px-3 py-6 text-xs text-muted-foreground text-center">
          {t("list.noDetected")}
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-0.5 p-2">
      <div className="px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
        {t("list.header")}
      </div>
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        modifiers={[restrictToVerticalAxis]}
        onDragEnd={handleDragEnd}
      >
        <SortableContext
          items={visible.map((a) => a.name)}
          strategy={verticalListSortingStrategy}
        >
          {visible.map((agent) => (
            <SortableAgentItem
              key={agent.name}
              agent={agent}
              isSelected={agent.name === selectedAgent}
              onSelect={() => selectAgent(agent.name)}
            />
          ))}
        </SortableContext>
      </DndContext>
    </div>
  );
}

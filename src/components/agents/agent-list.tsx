import { useCallback, useMemo } from "react";
import { clsx } from "clsx";
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { restrictToVerticalAxis } from "@dnd-kit/modifiers";
import { CSS } from "@dnd-kit/utilities";
import { GripVertical } from "lucide-react";
import { agentDisplayName } from "@/lib/types";
import type { AgentDetail } from "@/lib/types";
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
            : "text-muted-foreground/50"
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
        className="flex flex-col items-start flex-1 py-2.5 pr-3 text-left"
      >
        <span className="text-[13px] font-medium">{agentDisplayName(agent.name)}</span>
        {!agent.detected && (
          <span className="text-[10px] text-muted-foreground">Not detected</span>
        )}
      </button>
    </div>
  );
}

export function AgentList() {
  const agentDetails = useAgentConfigStore((s) => s.agentDetails);
  const selectedAgent = useAgentConfigStore((s) => s.selectedAgent);
  const selectAgent = useAgentConfigStore((s) => s.selectAgent);
  const agentOrder = useAgentStore((s) => s.agentOrder);
  const reorderAgents = useAgentStore((s) => s.reorderAgents);

  const sorted = useMemo(
    () => [...agentDetails].sort((a, b) => {
      const ai = agentOrder.indexOf(a.name);
      const bi = agentOrder.indexOf(b.name);
      return (ai === -1 ? 99 : ai) - (bi === -1 ? 99 : bi);
    }),
    [agentDetails, agentOrder],
  );

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const handleDragEnd = useCallback(
    (event: DragEndEvent) => {
      const { active, over } = event;
      if (!over || active.id === over.id) return;

      const names = sorted.map((a) => a.name);
      const oldIndex = names.indexOf(active.id as string);
      const newIndex = names.indexOf(over.id as string);
      if (oldIndex === -1 || newIndex === -1) return;

      reorderAgents(arrayMove(names, oldIndex, newIndex));
    },
    [sorted, reorderAgents],
  );

  return (
    <div className="flex flex-col gap-0.5 p-2">
      <div className="px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
        Agents
      </div>
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        modifiers={[restrictToVerticalAxis]}
        onDragEnd={handleDragEnd}
      >
        <SortableContext
          items={sorted.map((a) => a.name)}
          strategy={verticalListSortingStrategy}
        >
          {sorted.map((agent) => (
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

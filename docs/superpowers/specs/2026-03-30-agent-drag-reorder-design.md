# Agent Drag-to-Reorder — Design Spec

## Overview

Add drag-to-reorder to the Agents page left panel. The custom order persists in SQLite and applies globally across all UI surfaces (Overview, Extensions filters, etc.).

---

## Data Layer

### Schema Change

Add `sort_order INTEGER` column to the existing `agent_settings` table:

```sql
ALTER TABLE agent_settings ADD COLUMN sort_order INTEGER;
```

Default values match the current hardcoded `AGENT_ORDER`: claude=0, codex=1, gemini=2, cursor=3, antigravity=4, copilot=5. When `sort_order` is NULL (not yet set), fall back to the hardcoded default order.

### New Tauri Command

```rust
#[tauri::command]
fn update_agent_order(names: Vec<String>) -> Result<(), String>
```

Accepts an ordered list of agent names. Writes `sort_order = index` for each agent into `agent_settings`. This is a single transactional write.

### Backend Ordering

`list_agents` returns agents sorted by `sort_order` (falling back to hardcoded default for NULL values). The frontend trusts the backend order — no client-side re-sorting needed.

---

## Frontend Store

### agent-store changes

- `fetch()` no longer sorts by `AGENT_ORDER` — it uses the order returned by the backend
- New action: `reorderAgents(orderedNames: string[])` — optimistically reorders the local `agents` array, then calls `api.updateAgentOrder(orderedNames)` to persist

### Global sort function change

`sortAgents()` in `src/lib/types.ts` currently reads from the hardcoded `AGENT_ORDER` constant. Change it to accept an explicit order array parameter, and have callers pass the order from the agent store. This way all surfaces (Overview agent cards, Extensions agent filter, etc.) respect the user's custom order.

---

## UI — Agents Page Left Panel

### Drag Handle

Each agent list item gets a `GripVertical` icon (from `lucide-react`) as a drag handle, positioned at the left edge of the item. Styled with low opacity (`text-muted-foreground/30`) that increases on hover (`hover:text-muted-foreground/60`).

### Drag Library

Use `@dnd-kit/core` + `@dnd-kit/sortable` + `@dnd-kit/utilities`:
- `DndContext` wraps the agent list
- `SortableContext` with `verticalListSortingStrategy`
- Each agent item uses `useSortable` hook
- Drag handle uses the `listeners` and `attributes` from `useSortable`, attached only to the grip icon (not the whole row)

### Drag Behavior

- Drag restricted to vertical axis only (`restrictToVerticalAxis` modifier)
- On drag end: compute new order from `arrayMove`, call `reorderAgents()`
- Disabled (undetected) agents are still draggable — order applies to all agents regardless of detection status

### Visual Feedback

- Dragging item gets slight elevation/opacity change via CSS
- Drop placeholder handled by dnd-kit's built-in overlay

---

## Scope & Non-Goals

**In scope:**
- Drag handle + reorder in Agents page left panel
- Persist order in SQLite
- All UI surfaces respect custom order

**Not in scope:**
- Reorder from other pages (only Agents page has the drag UI)
- Reset to default order button (can be added later)
- Drag to reorder on mobile/touch (desktop app only)

import { useId, type ReactNode } from "react";
import {
  closestCenter,
  DndContext,
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
import { CSS } from "@dnd-kit/utilities";
import { GripVertical } from "lucide-react";

interface SortableItemProps {
  id: string;
  position: number;
  total: number;
  children: (args: { handle: ReactNode; dragging: boolean }) => ReactNode;
}

export function SortableItem({ id, children, position, total }: SortableItemProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id,
  });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  };

  const instructionsId = `sortable-instructions-${id}`;
  const positionLabel = total > 1 ? `, position ${position} of ${total}` : "";
  const handle = (
    <>
      <button
        ref={undefined}
        type="button"
        aria-label={`Drag to reorder item${positionLabel}. Press Space to lift. Use Arrow Up/Arrow Down to move, then press Space again to place.`}
        aria-describedby={instructionsId}
        {...attributes}
        {...listeners}
        className="touch-none cursor-grab active:cursor-grabbing text-neriak-dim hover:text-neriak-magenta"
      >
        <GripVertical className="w-3.5 h-3.5" strokeWidth={1.75} />
      </button>
      <span id={instructionsId} className="sr-only">
        Use Tab to move focus. Use Space to lift an item into keyboard move mode, then use Arrow
        Up/Down to reorder and Space to place it. Press Escape to cancel a drag.
      </span>
    </>
  );

  return (
    <div ref={setNodeRef} style={style}>
      {children({ handle, dragging: isDragging })}
    </div>
  );
}

interface SortableListProps<T extends { id: string }> {
  items: T[];
  onReorder: (items: T[]) => void;
  children: (item: T, index: number) => ReactNode;
}

export function SortableList<T extends { id: string }>({
  items,
  onReorder,
  children,
}: SortableListProps<T>) {
  const listId = useId();
  const instructionsId = `${listId}-instructions`;
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const onDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = items.findIndex((i) => i.id === active.id);
    const newIndex = items.findIndex((i) => i.id === over.id);
    if (oldIndex < 0 || newIndex < 0) return;
    onReorder(arrayMove(items, oldIndex, newIndex));
  };

  return (
    <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
      <SortableContext items={items.map((i) => i.id)} strategy={verticalListSortingStrategy}>
        <div className="space-y-1" aria-describedby={instructionsId} role="list">
          <span id={instructionsId} className="sr-only">
            Use Tab to move focus into this list. When focused on a drag handle, press Space to
            pick up and then Arrow Up/Arrow Down to reorder items.
          </span>
          {items.map((item, i) => (
            <SortableItem key={item.id} id={item.id} position={i + 1} total={items.length}>
              {() => <>{children(item, i)}</>}
            </SortableItem>
          ))}
        </div>
      </SortableContext>
    </DndContext>
  );
}

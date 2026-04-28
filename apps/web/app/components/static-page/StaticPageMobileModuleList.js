'use client';

import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import StaticPageModuleCard from './StaticPageModuleCard';

function SortableModuleItem({ module, onApplyOperation }) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: module.id });

  return (
    <article
      ref={setNodeRef}
      className={`static-page-mobile-module ${isDragging ? 'dragging' : ''}`.trim()}
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
      }}
    >
      <button
        type="button"
        className="static-page-mobile-module-handle"
        aria-label={`拖动 ${module.title}`}
        {...attributes}
        {...listeners}
      >
        <span></span>
        <span></span>
        <span></span>
      </button>
      <StaticPageModuleCard compact module={module} onApplyOperation={onApplyOperation} />
    </article>
  );
}

export default function StaticPageMobileModuleList({ draft, onReorder, onApplyOperation }) {
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
  const moduleMap = new Map(draft.modules.map((module) => [module.id, module]));
  const orderedIds = draft.mobileOrder?.length ? draft.mobileOrder : draft.modules.map((module) => module.id);
  const orderedModules = orderedIds.map((id) => moduleMap.get(id)).filter(Boolean);

  function handleDragEnd(event) {
    const { active, over } = event;
    if (!over || active.id === over.id) {
      return;
    }

    const oldIndex = orderedIds.indexOf(active.id);
    const newIndex = orderedIds.indexOf(over.id);
    if (oldIndex < 0 || newIndex < 0) {
      return;
    }

    onReorder?.(arrayMove(orderedIds, oldIndex, newIndex));
  }

  return (
    <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
      <SortableContext items={orderedIds} strategy={verticalListSortingStrategy}>
        <div className="static-page-mobile-module-list">
          {orderedModules.map((module) => (
            <SortableModuleItem key={module.id} module={module} onApplyOperation={onApplyOperation} />
          ))}
        </div>
      </SortableContext>
    </DndContext>
  );
}

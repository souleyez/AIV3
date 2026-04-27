'use client';

import { GridLayout, useContainerWidth } from 'react-grid-layout';
import StaticPageModuleCard from './StaticPageModuleCard';

function moduleLayout(module) {
  return {
    i: module.id,
    x: Number(module.layout?.x || 0),
    y: Number(module.layout?.y || 0),
    w: Number(module.layout?.w || 1),
    h: Number(module.layout?.h || 1),
    minW: 3,
    minH: 2,
  };
}

function layoutChanged(previous, next) {
  return previous.x !== next.x
    || previous.y !== next.y
    || previous.w !== next.w
    || previous.h !== next.h;
}

export default function StaticPagePlanningCanvas({ draft, onApplyOperation }) {
  const { width, containerRef, mounted } = useContainerWidth({ initialWidth: 680 });
  const layout = draft.modules.map(moduleLayout);

  function emitChangedLayout(_nextLayout, _oldItem, newItem, operationType) {
    const changed = newItem || _nextLayout.find((item) => {
      const module = draft.modules.find((candidate) => candidate.id === item.i);
      return module && layoutChanged(moduleLayout(module), item);
    });

    if (!changed) {
      return;
    }

    onApplyOperation?.({
      type: operationType,
      targetModuleId: changed.i,
      layout: {
        x: changed.x,
        y: changed.y,
        w: changed.w,
        h: changed.h,
      },
    });
  }

  return (
    <div className="static-page-canvas-shell" ref={containerRef}>
      {mounted ? (
        <GridLayout
          className="static-page-grid-layout"
          width={width}
          gridConfig={{
            cols: 12,
            rowHeight: 54,
            margin: [12, 12],
            containerPadding: [0, 0],
          }}
          dragConfig={{
            handle: '.static-page-module-drag-handle',
            bounded: true,
          }}
          resizeConfig={{
            handles: ['se'],
          }}
          layout={layout}
          onDragStop={(nextLayout, oldItem, newItem) => emitChangedLayout(nextLayout, oldItem, newItem, 'move_module')}
          onResizeStop={(nextLayout, oldItem, newItem) => emitChangedLayout(nextLayout, oldItem, newItem, 'resize_module')}
        >
          {draft.modules.map((module) => (
            <div key={module.id} className="static-page-grid-item">
              <StaticPageModuleCard module={module} />
            </div>
          ))}
        </GridLayout>
      ) : null}
    </div>
  );
}

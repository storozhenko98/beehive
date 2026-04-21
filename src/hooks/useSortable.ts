import React, { useState, useRef, useEffect, useCallback } from "react";

interface SortableItem {
  id: string;
}

interface UseSortableOptions {
  /** Pixels the pointer must move before drag activates (default: 5) */
  threshold?: number;
  /** CSS transition for items shifting into place (default: "transform 200ms cubic-bezier(0.2, 0, 0, 1)") */
  transition?: string;
}

interface SortableItemProps {
  ref: (el: HTMLDivElement | null) => void;
  onPointerDown: (e: React.PointerEvent) => void;
  style: React.CSSProperties | undefined;
  /** True if this specific item is being dragged */
  isDragged: boolean;
}

interface UseSortableReturn<T extends SortableItem> {
  /** Whether a drag is actively in progress */
  isDragging: boolean;
  /** Index of the item being dragged, or null */
  draggedIndex: number | null;
  /** Get props to spread onto each sortable item element */
  getItemProps: (index: number) => SortableItemProps;
  /** The items in their current order (same ref as input when not dragging) */
  items: T[];
}

interface ReorderMeta {
  from: number;
  to: number;
  movedId: string;
  clientY: number;
}

const DEFAULT_THRESHOLD = 5;
const DEFAULT_TRANSITION = "transform 200ms cubic-bezier(0.2, 0, 0, 1)";

function reorder<T>(list: T[], from: number, to: number): T[] {
  const result = [...list];
  const [moved] = result.splice(from, 1);
  result.splice(to, 0, moved);
  return result;
}

export function useSortable<T extends SortableItem>(
  items: T[],
  onReorder: (ids: string[], meta: ReorderMeta) => void,
  options?: UseSortableOptions,
): UseSortableReturn<T> {
  const threshold = options?.threshold ?? DEFAULT_THRESHOLD;
  const transition = options?.transition ?? DEFAULT_TRANSITION;

  const [dragState, setDragState] = useState<{
    dragIndex: number;
    overIndex: number;
    activated: boolean;
  } | null>(null);

  // Refs for stable access in window event listeners
  const dragRef = useRef(dragState);
  dragRef.current = dragState;
  const itemsRef = useRef(items);
  itemsRef.current = items;

  const pointerStartRef = useRef<{ x: number; y: number } | null>(null);
  const itemElsById = useRef<Map<string, HTMLDivElement>>(new Map());
  const itemRectsRef = useRef<{ id: string; mid: number }[]>([]);
  const dragItemHeightRef = useRef(40);

  function cacheItemRects() {
    const rects: { id: string; mid: number }[] = [];
    const ds = dragRef.current;
    for (let i = 0; i < itemsRef.current.length; i++) {
      const item = itemsRef.current[i];
      const el = itemElsById.current.get(item.id);
      if (el) {
        const rect = el.getBoundingClientRect();
        rects.push({ id: item.id, mid: rect.top + rect.height / 2 });
        if (ds && i === ds.dragIndex) dragItemHeightRef.current = rect.height;
      }
    }
    itemRectsRef.current = rects;
  }

  function findOverIndex(clientY: number): number {
    const rects = itemRectsRef.current;
    if (rects.length === 0) return 0;
    if (clientY < rects[0].mid) return 0;
    if (clientY >= rects[rects.length - 1].mid) return rects.length - 1;
    for (let i = 0; i < rects.length; i++) {
      if (clientY < rects[i].mid) return Math.max(0, i - 1);
    }
    return rects.length - 1;
  }

  function getTranslateY(i: number): number {
    if (!dragState || !dragState.activated) return 0;
    const { dragIndex, overIndex } = dragState;
    if (dragIndex === overIndex) return 0;

    const h = dragItemHeightRef.current;

    if (i === dragIndex) return (overIndex - dragIndex) * h;
    if (dragIndex < overIndex && i > dragIndex && i <= overIndex) return -h;
    if (dragIndex > overIndex && i >= overIndex && i < dragIndex) return h;
    return 0;
  }

  const handlePointerDown = useCallback((e: React.PointerEvent, index: number) => {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest("button")) return;
    if (e.shiftKey) return;

    e.preventDefault();
    pointerStartRef.current = { x: e.clientX, y: e.clientY };
    setDragState({ dragIndex: index, overIndex: index, activated: false });
  }, []);

  useEffect(() => {
    function onPointerMove(e: PointerEvent) {
      const ds = dragRef.current;
      if (!ds) return;
      const start = pointerStartRef.current;
      if (!start) return;

      if (!ds.activated) {
        const dx = e.clientX - start.x;
        const dy = e.clientY - start.y;
        if (Math.abs(dx) < threshold && Math.abs(dy) < threshold) return;
        cacheItemRects();
        setDragState((prev) => prev ? { ...prev, activated: true } : null);
        return;
      }

      const newOver = findOverIndex(e.clientY);
      if (newOver !== ds.overIndex) {
        setDragState((prev) => prev ? { ...prev, overIndex: newOver } : null);
      }
    }

    function onPointerUp(e: PointerEvent) {
      const ds = dragRef.current;
      if (!ds) return;
      if (ds.activated) {
        const reordered = reorder(itemsRef.current, ds.dragIndex, ds.overIndex);
        const movedItem = itemsRef.current[ds.dragIndex];
        onReorder(reordered.map((item) => item.id), {
          from: ds.dragIndex,
          to: ds.overIndex,
          movedId: movedItem.id,
          clientY: e.clientY,
        });
      }
      setDragState(null);
      pointerStartRef.current = null;
    }

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    return () => {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
    };
  }, [onReorder, threshold]);

  const isDragging = dragState?.activated ?? false;

  const getItemProps = useCallback((index: number): SortableItemProps => {
    const item = itemsRef.current[index];
    const isDragged = isDragging && index === dragRef.current?.dragIndex;
    const ty = getTranslateY(index);

    return {
      ref: (el: HTMLDivElement | null) => {
        if (item) {
          if (el) itemElsById.current.set(item.id, el);
          else itemElsById.current.delete(item.id);
        }
      },
      onPointerDown: (e: React.PointerEvent) => handlePointerDown(e, index),
      style: isDragging ? {
        transform: ty !== 0 ? `translateY(${ty}px)` : undefined,
        transition,
      } : undefined,
      isDragged,
    };
  }, [isDragging, dragState?.dragIndex, dragState?.overIndex, handlePointerDown, transition]);

  const draggedIndex = isDragging ? (dragState?.dragIndex ?? null) : null;

  return { isDragging, draggedIndex, getItemProps, items };
}

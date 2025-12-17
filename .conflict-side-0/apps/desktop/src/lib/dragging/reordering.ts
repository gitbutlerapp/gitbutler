/**
 * Stack re-ordering logic.
 *
 * The main idea behind the current re-ordering logic is that we update the
 * stack location as you are dragging it around, and then finalize it with
 * a call to the back end on drop.
 *
 * We also do not make use of drop zones, but instead use the onDrag event
 * to figure out when you have dragged a stack past the mid point of the
 * preceding, or next, stack.
 *
 * TODO: Make this into a svelte action that registers its own handlers?
 *
 * @example
 * <div class="container"
 *     ondragover={(e) => {
 *         onReorderDragOver(e, sortedStacks); // Mutates `sortedStacks`
 *     }}
 *     ondrop={() => { ... }}
 * >
 *     <div class="item" ondrop={() => updateOrder(sortedStacks)}>...</div>
 * </div>
 */
import { cloneElement } from '$lib/dragging/draggable';
import type { Stack } from '$lib/stacks/stack';

let dragHandle: HTMLElement | null;
let dragged: HTMLDivElement | undefined;
let clone: any;
let draggedId: string | undefined;

export function onReorderMouseDown(e: MouseEvent) {
	dragHandle = e.target as HTMLElement;
}

export function onReorderStart(
	e: DragEvent & { currentTarget: HTMLDivElement },
	stackId: string,
	callback?: () => void,
	preserveOriginalSize?: boolean
) {
	if (dragHandle?.dataset.dragHandle === undefined) {
		// Only elements with`data-drag-handle` attribute can initiate drag.
		// Note that elements inside the drag handle need `pointer-events: none`.
		e.preventDefault();
		e.stopPropagation();
		return;
	}

	callback?.();

	clone = cloneElement(e.currentTarget);
	document.body.appendChild(clone);
	// Get chromium to fire dragover & drop events
	// https://stackoverflow.com/questions/6481094/html5-drag-and-drop-ondragover-not-firing-in-chrome/6483205#6483205
	e.dataTransfer?.setData('text/html', 'd'); // cannot be empty string
	e.dataTransfer?.setDragImage(clone, e.offsetX, e.offsetY); // Adds the padding
	dragged = e.currentTarget;
	draggedId = stackId;
	dragged.style.opacity = '0.6';

	// additional styles to the clone to make background and border visible
	clone.style.position = 'absolute';
	clone.style.maxHeight = `${window.innerHeight - 100}px`;

	if (preserveOriginalSize) {
		// Preserve original dimensions (e.g., for collapsed lanes)
		const originalHeight = e.currentTarget.offsetHeight;
		const originalWidth = e.currentTarget.offsetWidth;
		clone.style.height = `${originalHeight}px`;
		clone.style.width = `${originalWidth}px`;
	} else {
		// For regular stacks, use auto height (existing behavior)
		clone.style.height = 'auto';
	}
	clone.style.zIndex = '-1';
	clone.style.top = '-10000px'; // Move it out of the way
	clone.style.left = '-10000px';
	clone.style.pointerEvents = 'none';
	clone.style.backgroundColor = 'var(--clr-bg-2)';
	clone.style.border = '1px solid var(--clr-border-2)';
	clone.style.borderRadius = 'var(--radius-ml)';
	clone.style.overflow = 'hidden';
}

export function onReorderEnd() {
	if (dragged) {
		dragged.style.opacity = '1';
		dragged = undefined;
		draggedId = undefined;
	}
	clone?.remove();
}

export function onDragOver(
	e: MouseEvent & { currentTarget: HTMLDivElement },
	sortedStacks: Stack[],
	thisStackId: string
) {
	// Return early if we are currently dragging over ourself.
	if (draggedId === thisStackId) {
		return;
	}

	const thisIdx = sortedStacks.findIndex((stack) => stack.id === thisStackId);
	const draggedIdx = sortedStacks.findIndex((stack) => stack.id === draggedId);
	if (draggedIdx === -1 || thisIdx === -1) {
		return;
	}

	// If we are dragging over an adjacent stack, only swap if the mouse is half
	// way over the adjacent stack.
	if (Math.abs(thisIdx - draggedIdx) === 1) {
		// The mouse position relative to the LHS of the current stack.
		const mouseLeft = e.clientX - (e.currentTarget?.getBoundingClientRect().left ?? 0);

		const isRightOfTarget = thisIdx > draggedIdx;

		const midpoint = (e.currentTarget?.clientWidth ?? 0) / 2;

		let pastOfMidpoint = false;
		if (isRightOfTarget) {
			pastOfMidpoint = mouseLeft > midpoint;
		} else {
			pastOfMidpoint = mouseLeft < midpoint;
		}

		if (pastOfMidpoint) {
			const draggedStack = sortedStacks.splice(draggedIdx, 1);
			sortedStacks.splice(thisIdx, 0, ...draggedStack);
		}
	} else {
		const draggedStack = sortedStacks.splice(draggedIdx, 1);
		sortedStacks.splice(thisIdx, 0, ...draggedStack);
	}
}

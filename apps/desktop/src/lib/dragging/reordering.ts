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

let dragHandle: any;
let dragged: HTMLDivElement | undefined;
let clone: any;
let draggedId: string | undefined;
let lanesScrollableEl: HTMLElement | undefined;

export function onReorderMouseDown(e: MouseEvent, element: HTMLDivElement | undefined) {
	dragHandle = e.target;
	lanesScrollableEl = element;
}

export function onReorderStart(
	e: DragEvent & { currentTarget: HTMLDivElement },
	stackId: string,
	callback?: () => void
) {
	if (dragHandle.dataset.dragHandle === undefined) {
		// Only elements with`data-drag-handle` attribute can initiate drag.
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
	clone.style.backgroundColor = 'var(--clr-bg-2)';
	clone.style.border = '1px solid var(--clr-border-2)';
	clone.style.borderRadius = 'var(--radius-ml)';
}

export function onReorderEnd() {
	if (dragged) {
		dragged.style.opacity = '1';
		dragged = undefined;
		draggedId = undefined;
	}
	clone?.remove();
}

export function onReorderDragOver(
	e: MouseEvent & { currentTarget: HTMLDivElement },
	sortedStacks: Stack[]
) {
	e.preventDefault();
	if (!dragged) {
		return; // Something other than a lane is being dragged.
	}

	const children = Array.from(e.currentTarget.children);
	const currentPosition = sortedStacks.findIndex((stack) => stack.id === draggedId);

	let dropPosition = 0;
	const mouseLeft = e.clientX - (lanesScrollableEl?.getBoundingClientRect().left ?? 0);
	let cumulativeWidth = lanesScrollableEl?.offsetLeft ?? 0;

	for (let i = 0; i < children.length; i++) {
		const childWidth = (children[i] as HTMLElement).offsetWidth;
		// The commented out code below is necessary if the drag handle is
		// aligned with the left side of the stack. Leaving it here until
		// we are more certain about the layout.
		// if (i === currentPosition) {
		// 	continue;
		// }
		if (mouseLeft > cumulativeWidth + childWidth / 2) {
			// New position depends on drag direction.
			dropPosition = i < currentPosition ? i + 1 : i;
			cumulativeWidth += childWidth;
		} else {
			break;
		}
	}

	// Update sorted branch array manually.
	if (currentPosition !== dropPosition) {
		const el = sortedStacks.splice(currentPosition, 1);
		sortedStacks.splice(dropPosition, 0, ...el);
	}
	return sortedStacks;
}

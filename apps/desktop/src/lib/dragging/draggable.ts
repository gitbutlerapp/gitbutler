import { type CommitStatusType } from '$lib/commits/commit';
import DragClone from '$lib/dragging/DragClone.svelte';
import { FileChangeDropData, type DropData } from '$lib/dragging/draggables';
import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
import { mount } from 'svelte';
import type { DropzoneRegistry } from '$lib/dragging/registry';
import type { PushStatus } from '$lib/stacks/stack';
import type { DragStateService } from '@gitbutler/ui/drag/dragStateService.svelte';

// Added to element being dragged (not the clone that follows the cursor).
const DRAGGING_CLASS = 'dragging';

// Minimum movement before drag starts (prevents accidental drags)
const MIN_MOVEMENT_BEFORE_DRAG_START_PX = 3;

// Observer interval for position tracking
const OBSERVATION_INTERVAL_MS = 16; // ~60fps

type chipType = 'file' | 'folder' | 'hunk' | 'ai-session' | 'branch';

type DragCloneType = 'branch' | 'commit' | 'file' | 'folder' | 'hunk' | 'ai-session';

interface DragCloneProps {
	type: DragCloneType;
	label?: string;
	filePath?: string;
	commitType?: CommitStatusType;
	childrenAmount?: number;
	pushStatus?: PushStatus;
	dragStateService?: DragStateService;
}

/**
 * Helper to create a drag clone using the Svelte DragClone component
 */
function createSvelteDragClone(props: DragCloneProps): HTMLElement {
	const container = document.createElement('div');
	mount(DragClone, {
		target: container,
		props
	});
	return container.firstElementChild as HTMLElement;
}

export type DraggableConfig = {
	readonly disabled?: boolean;
	readonly label?: string;
	readonly filePath?: string;
	readonly sha?: string;
	readonly date?: string;
	readonly authorImgUrl?: string;
	readonly commitType?: CommitStatusType;
	readonly data?: DropData;
	readonly viewportId?: string;
	readonly chipType?: chipType;
	readonly dropzoneRegistry: DropzoneRegistry;
	readonly dragStateService?: DragStateService;
	readonly pushStatus?: PushStatus;
};

function setupDragHandlers(
	node: HTMLElement,
	opts: DraggableConfig | undefined,
	createClone: (opts: DraggableConfig, selectedElements: Element[]) => HTMLElement | undefined,
	params: {
		handlerWidth: boolean;
		maxHeight?: number;
	} = {
		handlerWidth: false
	}
): {
	update: (opts: DraggableConfig) => void;
	destroy: () => void;
} {
	let dragHandle: HTMLElement | null = null;
	let clone: HTMLElement | undefined;
	let selectedElements: Element[] = [];
	let endDragging: (() => void) | undefined;

	// State management
	let isDragging = false;
	let dragStartPosition: { x: number; y: number } | null = null;
	let currentMousePosition: { x: number; y: number } | null = null;
	let observerInterval: ReturnType<typeof setInterval> | undefined;
	const scrollIntervals = new Map<HTMLElement, ReturnType<typeof setInterval>>();
	let currentHoveredDropzone: HTMLElement | null = null;

	// Auto-scroll detection
	const SCROLL_EDGE_SIZE = 50;
	const SCROLL_SPEED = 10;

	function findScrollableContainers(element: HTMLElement): HTMLElement[] {
		const containers: HTMLElement[] = [];
		let current: HTMLElement | null = element;

		while (current && current !== document.body) {
			const style = window.getComputedStyle(current);
			const overflowY = style.overflowY;
			const overflowX = style.overflowX;

			if (
				(overflowY === 'auto' ||
					overflowY === 'scroll' ||
					overflowX === 'auto' ||
					overflowX === 'scroll') &&
				(current.scrollHeight > current.clientHeight || current.scrollWidth > current.clientWidth)
			) {
				containers.push(current);
			}

			current = current.parentElement;
		}

		// Add window scrolling if page is scrollable
		if (document.documentElement.scrollHeight > window.innerHeight) {
			containers.push(document.documentElement);
		}

		return containers;
	}

	function handleAutoScroll(mouseX: number, mouseY: number) {
		if (!currentMousePosition) return;

		const scrollContainers = findScrollableContainers(node);

		scrollContainers.forEach((container) => {
			const rect = container.getBoundingClientRect();
			let scrollX = 0;
			let scrollY = 0;

			// Check vertical scrolling
			if (mouseY < rect.top + SCROLL_EDGE_SIZE && container.scrollTop > 0) {
				scrollY = -SCROLL_SPEED;
			} else if (
				mouseY > rect.bottom - SCROLL_EDGE_SIZE &&
				container.scrollTop < container.scrollHeight - container.clientHeight
			) {
				scrollY = SCROLL_SPEED;
			}

			// Check horizontal scrolling
			if (mouseX < rect.left + SCROLL_EDGE_SIZE && container.scrollLeft > 0) {
				scrollX = -SCROLL_SPEED;
			} else if (
				mouseX > rect.right - SCROLL_EDGE_SIZE &&
				container.scrollLeft < container.scrollWidth - container.clientWidth
			) {
				scrollX = SCROLL_SPEED;
			}

			// Start or stop scroll interval for this container
			if (scrollX !== 0 || scrollY !== 0) {
				if (!scrollIntervals.has(container)) {
					const interval = setInterval(() => {
						if (container === document.documentElement) {
							window.scrollBy(scrollX, scrollY);
						} else {
							container.scrollBy(scrollX, scrollY);
						}
					}, 16); // ~60fps
					scrollIntervals.set(container, interval);
				}
			} else if (scrollIntervals.has(container)) {
				clearInterval(scrollIntervals.get(container));
				scrollIntervals.delete(container);
			}
		});
	}

	function stopAutoScroll() {
		scrollIntervals.forEach((interval) => clearInterval(interval));
		scrollIntervals.clear();
	}

	function handleMouseDown(e: MouseEvent) {
		if (!opts || opts.disabled) return;

		// Only left click
		if (e.button !== 0) return;

		// Check if clicking on a drag handle
		const target = e.target as HTMLElement;
		if (target.dataset.noDrag !== undefined) return;

		// Prevent dragging from input elements
		if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
			return;
		}

		e.preventDefault();
		e.stopPropagation();

		dragHandle = target;
		dragStartPosition = { x: e.clientX, y: e.clientY };
		currentMousePosition = { x: e.clientX, y: e.clientY };

		// Add listeners for potential drag
		window.addEventListener('mousemove', handleMouseMoveMaybeStart, { passive: false });
		window.addEventListener('mouseup', handleMouseUpBeforeDrag, { passive: false });
	}

	function handleMouseMoveMaybeStart(e: MouseEvent) {
		if (!dragStartPosition || !dragHandle) return;

		e.preventDefault();
		currentMousePosition = { x: e.clientX, y: e.clientY };

		// Check if moved enough to start drag
		const dx = Math.abs(e.clientX - dragStartPosition.x);
		const dy = Math.abs(e.clientY - dragStartPosition.y);

		if (dx >= MIN_MOVEMENT_BEFORE_DRAG_START_PX || dy >= MIN_MOVEMENT_BEFORE_DRAG_START_PX) {
			// Remove maybe listeners
			window.removeEventListener('mousemove', handleMouseMoveMaybeStart);
			window.removeEventListener('mouseup', handleMouseUpBeforeDrag);

			// Start actual drag
			startDrag(e);
		}
	}

	function handleMouseUpBeforeDrag() {
		// User released before moving enough - cancel
		window.removeEventListener('mousemove', handleMouseMoveMaybeStart);
		window.removeEventListener('mouseup', handleMouseUpBeforeDrag);
		dragHandle = null;
		dragStartPosition = null;
		currentMousePosition = null;
	}

	function startDrag(e: MouseEvent) {
		if (!opts || !dragStartPosition) return;

		isDragging = true;

		const parentNode = node.parentElement?.parentElement;
		if (!parentNode) {
			console.error('draggable parent node not found');
			return;
		}

		// Start drag state tracking
		if (opts.dragStateService) {
			endDragging = opts.dragStateService.startDragging();
		}

		// Handle multi-selection for files
		if (opts.data instanceof FileChangeDropData) {
			selectedElements = [];
			for (const path of opts.data.changedPaths(opts.data.selectionId)) {
				const element = parentNode.querySelector(`[data-file-id="${path}"]`);
				if (element) {
					selectedElements.push(element);
				}
			}
		}

		if (selectedElements.length === 0) {
			selectedElements = [node];
		}

		// Mark elements as dragging
		for (const element of selectedElements) {
			element.classList.add(DRAGGING_CLASS);
		}

		// Activate dropzones
		for (const dropzone of Array.from(opts.dropzoneRegistry.values())) {
			dropzone.activate(opts.data);
		}

		// Create drag clone
		clone = createClone(opts, selectedElements);
		if (clone) {
			if (params.handlerWidth) {
				clone.style.width = node.clientWidth + 'px';
			}
			if (params.maxHeight) {
				clone.style.maxHeight = `${pxToRem(params.maxHeight)}rem`;
			}

			// Position clone at cursor with GPU-accelerated transform
			clone.style.position = 'fixed';
			clone.style.left = '0';
			clone.style.top = '0';
			clone.style.transform = `translate(${e.clientX}px, ${e.clientY}px) translate(-50%, -50%)`;
			clone.style.pointerEvents = 'none';
			clone.style.zIndex = 'var(--z-blocker)';
			clone.style.willChange = 'transform';

			document.body.appendChild(clone);
		}

		// Add drag listeners
		window.addEventListener('mousemove', handleMouseMove, { passive: false });
		window.addEventListener('mouseup', handleMouseUp, { passive: false });

		// Start position observer
		startObserver();
	}

	function handleMouseMove(e: MouseEvent) {
		if (!isDragging) return;

		e.preventDefault();
		currentMousePosition = { x: e.clientX, y: e.clientY };

		// Update clone position
		if (clone) {
			clone.style.transform = `translate(${e.clientX}px, ${e.clientY}px) translate(-50%, -50%)`;
		}

		// Handle auto-scrolling
		handleAutoScroll(e.clientX, e.clientY);
	}

	function handleMouseUp(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();

		cleanup();
	}

	function startObserver() {
		// Continuous position tracking for better dropzone detection
		observerInterval = setInterval(() => {
			if (!currentMousePosition || !opts) return;

			const { x, y } = currentMousePosition;
			const elementUnderCursor = document.elementFromPoint(x, y);

			let foundDropzone: HTMLElement | null = null;

			if (elementUnderCursor) {
				// Check if we're over a dropzone
				for (const [dzElement, dropzone] of opts.dropzoneRegistry.entries()) {
					const target = dropzone.getTarget();
					const rect = target.getBoundingClientRect();
					const isInside = x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;

					if (isInside && (target.contains(elementUnderCursor) || target === elementUnderCursor)) {
						foundDropzone = dzElement;
						break;
					}
				}
			}

			// Handle dropzone enter/leave
			if (foundDropzone !== currentHoveredDropzone) {
				// Leave previous dropzone
				if (currentHoveredDropzone) {
					const dropzone = opts.dropzoneRegistry.get(currentHoveredDropzone);
					dropzone?.triggerLeave();
				}

				// Enter new dropzone
				if (foundDropzone) {
					const dropzone = opts.dropzoneRegistry.get(foundDropzone);
					dropzone?.triggerEnter();
				}

				currentHoveredDropzone = foundDropzone;
			}
		}, OBSERVATION_INTERVAL_MS);
	}

	function stopObserver() {
		if (observerInterval) {
			clearInterval(observerInterval);
			observerInterval = undefined;
		}
	}

	function cleanup() {
		isDragging = false;

		// Send final dragleave to any hovered dropzone
		if (currentHoveredDropzone && opts) {
			const dropzone = opts.dropzoneRegistry.get(currentHoveredDropzone);
			dropzone?.triggerLeave();
			currentHoveredDropzone = null;
		}

		// Remove listeners
		window.removeEventListener('mousemove', handleMouseMove);
		window.removeEventListener('mouseup', handleMouseUp);
		window.removeEventListener('mousemove', handleMouseMoveMaybeStart);
		window.removeEventListener('mouseup', handleMouseUpBeforeDrag);

		// Stop observer
		stopObserver();

		// Stop auto-scroll
		stopAutoScroll();

		// Deactivate dropzones
		selectedElements.forEach((el) => el.classList.remove(DRAGGING_CLASS));

		if (clone) {
			clone.remove();
			clone = undefined;
		}

		if (opts) {
			Array.from(opts.dropzoneRegistry.values()).forEach((dropzone) => {
				dropzone.deactivate();
			});
		}

		// End drag state tracking
		endDragging?.();
		endDragging = undefined;

		// Reset state
		dragHandle = null;
		dragStartPosition = null;
		currentMousePosition = null;
		selectedElements = [];
	}

	function setup(newOpts: DraggableConfig) {
		if (newOpts.disabled) return;
		opts = newOpts;

		node.addEventListener('mousedown', handleMouseDown, { passive: false });
	}

	function clean() {
		cleanup();
		node.removeEventListener('mousedown', handleMouseDown);
	}

	if (opts) {
		setup(opts);
	}

	return {
		update(newOpts: DraggableConfig) {
			clean();
			setup(newOpts);
		},
		destroy() {
			clean();
		}
	};
}

/////////////////////////////
//// BRANCH DRAGGABLE ///////
/////////////////////////////

export function draggableBranch(node: HTMLElement, initialOpts: DraggableConfig) {
	function createClone(opts: DraggableConfig) {
		if (opts.disabled) return;
		return createSvelteDragClone({
			type: 'branch',
			label: opts.label,
			pushStatus: opts.pushStatus,
			dragStateService: opts.dragStateService
		});
	}
	return setupDragHandlers(node, initialOpts, createClone, {
		handlerWidth: false
	});
}

/////////////////////////////
//// COMMIT DRAGGABLE V3 ////
/////////////////////////////

export function draggableCommitV3(node: HTMLElement, initialOpts: DraggableConfig) {
	function createClone(opts: DraggableConfig) {
		if (opts.disabled) return;
		return createSvelteDragClone({
			type: 'commit',
			commitType: opts.commitType,
			label: opts.label,
			dragStateService: opts.dragStateService
		});
	}
	return setupDragHandlers(node, initialOpts, createClone, {
		handlerWidth: false
	});
}

export function draggableChips(node: HTMLElement, initialOpts: DraggableConfig) {
	function createClone(opts: DraggableConfig, selectedElements: Element[]) {
		const chipType = opts.chipType || 'file';
		return createSvelteDragClone({
			type: chipType as DragCloneType,
			childrenAmount: selectedElements.length,
			label: opts.label,
			filePath: opts.filePath,
			dragStateService: opts.dragStateService
		});
	}
	return setupDragHandlers(node, initialOpts, createClone);
}

////////////////////////////
//// GENERAL DRAG CLONE ////
////////////////////////////

export function cloneElement(node: HTMLElement) {
	const cloneEl = node.cloneNode(true) as HTMLElement;

	// exclude all ignored elements from the clone
	const ignoredElements = Array.from(cloneEl.querySelectorAll('[data-drag-clone-ignore]'));
	ignoredElements.forEach((el) => el.remove());

	return cloneEl;
}

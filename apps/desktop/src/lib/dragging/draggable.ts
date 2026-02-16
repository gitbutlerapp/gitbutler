import { type CommitStatusType } from "$lib/commits/commit";
import DragClone from "$lib/dragging/DragClone.svelte";
import { FileChangeDropData, type DropData } from "$lib/dragging/draggables";
import { pxToRem } from "@gitbutler/ui/utils/pxToRem";
import { mount } from "svelte";
import type { DropzoneRegistry } from "$lib/dragging/registry";
import type { PushStatus } from "$lib/stacks/stack";
import type { DragStateService } from "@gitbutler/ui/drag/dragStateService.svelte";

// Added to element being dragged (not the clone that follows the cursor).
const DRAGGING_CLASS = "dragging";

// Minimum movement before drag starts (prevents accidental drags)
const MIN_MOVEMENT_BEFORE_DRAG_START_PX = 3;

type chipType = "file" | "folder" | "hunk" | "ai-session" | "branch";

type DragCloneType = "branch" | "commit" | "file" | "folder" | "hunk" | "ai-session";

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
	const container = document.createElement("div");
	mount(DragClone, {
		target: container,
		props,
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
		handlerWidth: false,
	},
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
	let observerAnimationFrame: number | undefined;
	let currentHoveredDropzone: HTMLElement | null = null;
	let cachedScrollContainers: HTMLElement[] = [];

	// Auto-scroll detection
	const SCROLL_EDGE_SIZE = 50;
	const SCROLL_SPEED = 10;

	// Auto-scroll optimization: cache container rects and throttle updates
	// Reduces getBoundingClientRect() calls from ~60/sec to ~10/sec, avoiding layout thrashing
	const SCROLL_RECT_UPDATE_INTERVAL_MS = 100;
	const cachedScrollRects: Map<HTMLElement, DOMRect> = new Map();
	let lastScrollRectUpdate = 0;

	/**
	 * Check if an element has scrollable overflow.
	 */
	function hasScrollableOverflow(style: CSSStyleDeclaration): boolean {
		return (
			style.overflowY === "auto" ||
			style.overflowY === "scroll" ||
			style.overflowX === "auto" ||
			style.overflowX === "scroll"
		);
	}

	/**
	 * Check if an element is actually scrollable (has overflow and content exceeds bounds).
	 */
	function isScrollable(element: HTMLElement): boolean {
		const style = window.getComputedStyle(element);
		return (
			hasScrollableOverflow(style) &&
			(element.scrollHeight > element.clientHeight || element.scrollWidth > element.clientWidth)
		);
	}

	function findScrollableContainers(element: HTMLElement): HTMLElement[] {
		const containers: HTMLElement[] = [];
		const addedContainers = new Set<HTMLElement>();

		// 1. Find all explicitly marked scrollable containers for dragging
		const markedContainers = document.querySelectorAll("[data-scrollable-for-dragging]");
		for (const container of markedContainers) {
			if (!(container instanceof HTMLElement)) continue;

			if (isScrollable(container)) {
				const rect = container.getBoundingClientRect();
				if (isContainerVisibleInViewport(rect)) {
					containers.push(container);
					addedContainers.add(container);
				}
			}
		}

		// 2. Also include parent chain (for backwards compatibility and nested scrolling)
		let current: HTMLElement | null = element;
		while (current && current !== document.body) {
			if (!addedContainers.has(current) && isScrollable(current)) {
				containers.push(current);
				addedContainers.add(current);
			}
			current = current.parentElement;
		}

		// 3. Add window scrolling if page is scrollable
		if (document.documentElement.scrollHeight > window.innerHeight) {
			containers.push(document.documentElement);
		}

		return containers;
	}

	/**
	 * Check if a container is visible in the viewport.
	 * Containers outside viewport cannot be auto-scrolled via mouse position.
	 */
	function isContainerVisibleInViewport(rect: DOMRect): boolean {
		return (
			rect.bottom > 0 &&
			rect.top < window.innerHeight &&
			rect.right > 0 &&
			rect.left < window.innerWidth
		);
	}

	function performAutoScroll(mouseX: number, mouseY: number) {
		const now = performance.now();

		// Throttle rect updates: refresh every 100ms instead of every frame (~16ms)
		// This dramatically reduces layout recalculation during drag operations
		if (now - lastScrollRectUpdate > SCROLL_RECT_UPDATE_INTERVAL_MS) {
			cachedScrollRects.clear();
			cachedScrollContainers.forEach((container) => {
				const rect = container.getBoundingClientRect();
				// Only cache visible containers - off-screen containers can't be scrolled via mouse
				if (isContainerVisibleInViewport(rect)) {
					cachedScrollRects.set(container, rect);
				}
			});
			lastScrollRectUpdate = now;
		}

		// Find the innermost (most specific) scrollable container under the cursor
		// This ensures only one container scrolls, preventing nested containers from scrolling together
		const elementUnderCursor = document.elementFromPoint(mouseX, mouseY);
		let targetContainer: HTMLElement | null = null;
		let targetRect: DOMRect | null = null;

		if (elementUnderCursor) {
			let node: HTMLElement | null = elementUnderCursor as HTMLElement;
			while (node) {
				if (cachedScrollRects.has(node)) {
					targetContainer = node;
					targetRect = cachedScrollRects.get(node)!;
					break;
				}
				node = node.parentElement;
			}
		}

		// If no scrollable container found under cursor, nothing to scroll
		if (!targetContainer || !targetRect) return;

		let scrollX = 0;
		let scrollY = 0;

		// Check vertical scrolling
		if (mouseY < targetRect.top + SCROLL_EDGE_SIZE && targetContainer.scrollTop > 0) {
			scrollY = -SCROLL_SPEED;
		} else if (
			mouseY > targetRect.bottom - SCROLL_EDGE_SIZE &&
			targetContainer.scrollTop < targetContainer.scrollHeight - targetContainer.clientHeight
		) {
			scrollY = SCROLL_SPEED;
		}

		// Check horizontal scrolling
		if (mouseX < targetRect.left + SCROLL_EDGE_SIZE && targetContainer.scrollLeft > 0) {
			scrollX = -SCROLL_SPEED;
		} else if (
			mouseX > targetRect.right - SCROLL_EDGE_SIZE &&
			targetContainer.scrollLeft < targetContainer.scrollWidth - targetContainer.clientWidth
		) {
			scrollX = SCROLL_SPEED;
		}

		// Perform scroll if needed
		if (scrollX !== 0 || scrollY !== 0) {
			if (targetContainer === document.documentElement) {
				window.scrollBy(scrollX, scrollY);
			} else {
				targetContainer.scrollBy(scrollX, scrollY);
			}
		}
	}

	function handleMouseDown(e: MouseEvent) {
		if (!opts || opts.disabled) return;

		// Only left click
		if (e.button !== 0) return;

		// Check if clicking on a drag handle
		const target = e.target as HTMLElement;
		if (target.dataset.noDrag !== undefined) return;

		// Prevent dragging from input elements
		if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable) {
			return;
		}

		e.preventDefault();
		e.stopPropagation();

		dragHandle = target;
		dragStartPosition = { x: e.clientX, y: e.clientY };
		currentMousePosition = { x: e.clientX, y: e.clientY };

		// Add listeners for potential drag - using passive listeners
		window.addEventListener("mousemove", handleMouseMoveMaybeStart);
		window.addEventListener("mouseup", handleMouseUpBeforeDrag);
	}

	function handleMouseMoveMaybeStart(e: MouseEvent) {
		if (!dragStartPosition || !dragHandle) return;

		currentMousePosition = { x: e.clientX, y: e.clientY };

		// Check if moved enough to start drag
		const dx = Math.abs(e.clientX - dragStartPosition.x);
		const dy = Math.abs(e.clientY - dragStartPosition.y);

		if (dx >= MIN_MOVEMENT_BEFORE_DRAG_START_PX || dy >= MIN_MOVEMENT_BEFORE_DRAG_START_PX) {
			// Remove maybe listeners
			window.removeEventListener("mousemove", handleMouseMoveMaybeStart);
			window.removeEventListener("mouseup", handleMouseUpBeforeDrag);

			// Start actual drag
			startDrag(e);
		}
	}

	function handleMouseUpBeforeDrag() {
		// User released before moving enough - cancel
		window.removeEventListener("mousemove", handleMouseMoveMaybeStart);
		window.removeEventListener("mouseup", handleMouseUpBeforeDrag);

		dragHandle = null;
		dragStartPosition = null;
		currentMousePosition = null;
	}

	function startDrag(e: MouseEvent) {
		if (!opts || !dragStartPosition) return;

		isDragging = true;

		const parentNode = node.parentElement?.parentElement;
		if (!parentNode) {
			console.error("draggable parent node not found");
			return;
		}

		// Start drag state tracking
		// Cache scrollable containers once at drag start (not on every mousemove)
		cachedScrollContainers = findScrollableContainers(node);

		// Reset auto-scroll optimization state
		cachedScrollRects.clear();
		lastScrollRectUpdate = 0;

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
				clone.style.width = node.clientWidth + "px";
			}
			if (params.maxHeight) {
				clone.style.maxHeight = `${pxToRem(params.maxHeight)}rem`;
			}

			// Position clone at cursor with GPU-accelerated transform
			clone.style.position = "fixed";
			clone.style.left = "0";
			clone.style.top = "0";
			clone.style.transform = `translate(${e.clientX}px, ${e.clientY}px) translate(-50%, -50%)`;
			clone.style.pointerEvents = "none";
			clone.style.zIndex = "var(--z-blocker)";

			document.body.appendChild(clone);
		}

		// Add drag listeners - mousemove is passive, mouseup needs passive:false for preventDefault
		window.addEventListener("mousemove", handleMouseMove);
		window.addEventListener("mouseup", handleMouseUp, { passive: false });

		// Start position observer
		startObserver();
	}

	function handleMouseMove(e: MouseEvent) {
		if (!isDragging) return;

		currentMousePosition = { x: e.clientX, y: e.clientY };

		// Update clone position with GPU-accelerated transform
		if (clone) {
			clone.style.transform = `translate(${e.clientX}px, ${e.clientY}px) translate(-50%, -50%)`;
		}
	}

	function handleMouseUp(e: MouseEvent) {
		if (opts) {
			Array.from(opts.dropzoneRegistry.values()).forEach((dropzone) => {
				dropzone.deactivate();
			});
		}
		e.preventDefault();
		e.stopPropagation();

		cleanup();
	}

	function startObserver() {
		// Continuous position tracking and auto-scroll synchronized with browser repaints
		function observe() {
			if (!isDragging || !currentMousePosition || !opts) {
				return;
			}

			const { x, y } = currentMousePosition;

			// Perform auto-scrolling (synced with RAF)
			performAutoScroll(x, y);

			let foundDropzone: HTMLElement | null = null;
			const elementUnderCursor = document.elementFromPoint(x, y) as HTMLElement | null;

			if (elementUnderCursor) {
				// Optimized dropzone detection: walk up from the element under the cursor
				// and find the first ancestor that is a registered dropzone. This avoids
				// scanning all dropzones and eliminates expensive getBoundingClientRect()
				// calls on every animation frame.
				let node: HTMLElement | null = elementUnderCursor;

				while (node) {
					if (opts.dropzoneRegistry.has(node)) {
						foundDropzone = node;
						break;
					}

					node = node.parentElement;
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

			// Schedule next observation
			observerAnimationFrame = requestAnimationFrame(observe);
		}

		// Start the observation loop
		observerAnimationFrame = requestAnimationFrame(observe);
	}

	function stopObserver() {
		if (observerAnimationFrame) {
			cancelAnimationFrame(observerAnimationFrame);
			observerAnimationFrame = undefined;
		}
	}

	function cleanup() {
		// Send final dragleave to any hovered dropzone
		if (currentHoveredDropzone && opts) {
			const dropzone = opts.dropzoneRegistry.get(currentHoveredDropzone);
			dropzone?.triggerLeave();
			currentHoveredDropzone = null;
		}

		// Remove listeners
		if (isDragging) {
			window.removeEventListener("mousemove", handleMouseMove);
			window.removeEventListener("mouseup", handleMouseUp);
		}
		window.removeEventListener("mousemove", handleMouseMoveMaybeStart);
		window.removeEventListener("mouseup", handleMouseUpBeforeDrag);

		isDragging = false;

		// Stop observer (also stops auto-scroll since it's in the same RAF loop)
		stopObserver();

		// Deactivate dropzones
		selectedElements.forEach((el) => el.classList.remove(DRAGGING_CLASS));

		if (clone) {
			clone.remove();
			clone = undefined;
		}

		// End drag state tracking
		endDragging?.();
		endDragging = undefined;

		// Reset state
		dragHandle = null;
		cachedScrollContainers = [];
		cachedScrollRects.clear();
		lastScrollRectUpdate = 0;
		dragStartPosition = null;
		currentMousePosition = null;
		selectedElements = [];
	}

	function setup(newOpts: DraggableConfig) {
		if (newOpts.disabled) return;
		opts = newOpts;

		// Mousedown needs passive:false because we call preventDefault
		node.addEventListener("mousedown", handleMouseDown, { passive: false });
	}

	function clean() {
		cleanup();
		node.removeEventListener("mousedown", handleMouseDown);
	}

	if (opts) {
		setup(opts);
	}

	return {
		update(newOpts: DraggableConfig) {
			setup(newOpts);
		},
		destroy() {
			clean();
		},
	};
}

/////////////////////////////
//// BRANCH DRAGGABLE ///////
/////////////////////////////

export function draggableBranch(node: HTMLElement, initialOpts: DraggableConfig) {
	function createClone(opts: DraggableConfig) {
		if (opts.disabled) return;
		return createSvelteDragClone({
			type: "branch",
			label: opts.label,
			pushStatus: opts.pushStatus,
			dragStateService: opts.dragStateService,
		});
	}
	return setupDragHandlers(node, initialOpts, createClone, {
		handlerWidth: false,
	});
}

/////////////////////////////
//// COMMIT DRAGGABLE V3 ////
/////////////////////////////

export function draggableCommitV3(node: HTMLElement, initialOpts: DraggableConfig) {
	function createClone(opts: DraggableConfig) {
		if (opts.disabled) return;
		return createSvelteDragClone({
			type: "commit",
			commitType: opts.commitType,
			label: opts.label,
			dragStateService: opts.dragStateService,
		});
	}
	return setupDragHandlers(node, initialOpts, createClone, {
		handlerWidth: false,
	});
}

export function draggableChips(node: HTMLElement, initialOpts: DraggableConfig) {
	function createClone(opts: DraggableConfig, selectedElements: Element[]) {
		const chipType = opts.chipType || "file";
		return createSvelteDragClone({
			type: chipType as DragCloneType,
			childrenAmount: selectedElements.length,
			label: opts.label,
			filePath: opts.filePath,
			dragStateService: opts.dragStateService,
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
	const ignoredElements = Array.from(cloneEl.querySelectorAll("[data-drag-clone-ignore]"));
	ignoredElements.forEach((el) => el.remove());

	return cloneEl;
}

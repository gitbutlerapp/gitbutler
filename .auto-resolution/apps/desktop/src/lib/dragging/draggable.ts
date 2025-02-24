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

type chipType = 'file' | 'folder' | 'hunk' | 'ai-session' | 'branch';

type DragCloneType = 'branch' | 'commit' | 'file' | 'folder' | 'hunk' | 'ai-session';

interface DragCloneProps {
	type: DragCloneType;
	label?: string;
	filePath?: string;
	commitType?: CommitStatusType;
	childrenAmount?: number;
	pushStatus?: PushStatus;
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
	let dragHandle: HTMLElement | null;
	let clone: HTMLElement | undefined;
	let selectedElements: Element[] = [];
	let endDragging: (() => void) | undefined;

	function handleMouseDown(e: MouseEvent) {
		dragHandle = e.target as HTMLElement;
	}

	function handleDragStart(e: DragEvent) {
		if (!opts || opts.disabled) return;
		e.stopPropagation();

		if (!dragHandle || dragHandle.dataset.noDrag !== undefined) {
			e.preventDefault();
			return false;
		}

		const parentNode = node.parentElement?.parentElement;
		if (!parentNode) {
			console.error('draggable parent node not found');
			return;
		}

		// Start drag state tracking
		if (opts.dragStateService) {
			endDragging = opts.dragStateService.startDragging();
		}

		if (opts.data instanceof FileChangeDropData) {
			selectedElements = [];
			for (const path of opts.data.changedPaths(opts.data.selectionId)) {
				// Path is sufficient as a key since we query the parent container.
				const element = parentNode.querySelector(`[data-file-id="${path}"]`);
				if (element) {
					selectedElements.push(element);
				}
			}
		}

		if (selectedElements.length === 0) {
			selectedElements = [node];
		}

		for (const element of selectedElements) {
			element.classList.add(DRAGGING_CLASS);
		}

		for (const dropzone of Array.from(opts.dropzoneRegistry.values())) {
			dropzone.activate(opts.data);
		}

		clone = createClone(opts, selectedElements);
		if (clone) {
			// TODO: remove params (clientWidth, maxHeight) V3 design has shipped.
			if (params.handlerWidth) {
				clone.style.width = node.clientWidth + 'px';
			}
			if (params.maxHeight) {
				clone.style.maxHeight = `${pxToRem(params.maxHeight)}rem`;
			}
			document.body.appendChild(clone);

			if (e.dataTransfer) {
				if (params.handlerWidth) {
					e.dataTransfer.setDragImage(clone, e.offsetX, e.offsetY);
				} else {
					// Use more centered positioning for consistent cursor placement
					e.dataTransfer.setDragImage(clone, clone.offsetWidth / 1.2, clone.offsetHeight / 1.5);
				}

				// Get chromium to fire dragover & drop events
				// https://stackoverflow.com/questions/6481094/html5-drag-and-drop-ondragover-not-firing-in-chrome/6483205#6483205
				e.dataTransfer?.setData('text/html', 'd'); // cannot be empty string
				e.dataTransfer.effectAllowed = 'uninitialized';
			}
		}
	}

	const viewport = opts?.viewportId ? document.getElementById(opts.viewportId) : null;
	const triggerRange = 150;
	const timerShutter = 700;
	let timeoutId: undefined | ReturnType<typeof setTimeout> = undefined;

	function loopScroll(viewport: HTMLElement, direction: 'left' | 'right', scrollSpeed: number) {
		viewport.scrollBy({
			left: direction === 'left' ? -scrollSpeed : scrollSpeed,
			behavior: 'smooth'
		});

		timeoutId = setTimeout(() => loopScroll(viewport, direction, scrollSpeed), timerShutter);
	}

	function handleDrag(e: DragEvent) {
		e.preventDefault();

		if (!viewport) return;

		const scrollSpeed = (viewport.clientWidth || 500) / 3; // Fine-tune the scroll speed
		const viewportWidth = viewport.clientWidth;
		const relativeX = e.clientX - viewport.getBoundingClientRect().left;

		if (relativeX < triggerRange && viewport.scrollLeft > 0) {
			// Start scrolling to the left
			if (!timeoutId) {
				loopScroll(viewport, 'left', scrollSpeed);
			}
		} else if (relativeX > viewportWidth - triggerRange) {
			// Start scrolling to the right
			if (!timeoutId) {
				loopScroll(viewport, 'right', scrollSpeed);
			}
		} else {
			// Stop scrolling if not in the scrollable range
			if (timeoutId) {
				clearTimeout(timeoutId);
				timeoutId = undefined;
			}
		}
	}

	function handleDragEnd(e: DragEvent) {
		dragHandle = null;
		e.stopPropagation();
		deactivateDropzones();

		// End drag state tracking
		endDragging?.();
	}

	function setup(newOpts: DraggableConfig) {
		if (newOpts.disabled) return;
		opts = newOpts;
		node.draggable = true;

		node.addEventListener('dragstart', handleDragStart);
		node.addEventListener('drag', handleDrag);
		node.addEventListener('dragend', handleDragEnd);
		node.addEventListener('mousedown', handleMouseDown, { capture: false });
	}

	function clean() {
		if (dragHandle) {
			// If drop handler is updated/destroyed before drag end.
			deactivateDropzones();
		}

		node.draggable = false;
		node.removeEventListener('dragstart', handleDragStart);
		node.removeEventListener('drag', handleDrag);
		node.removeEventListener('dragend', handleDragEnd);
		node.removeEventListener('mousedown', handleMouseDown, { capture: false });

		// Clean up drag state if not already done
		endDragging?.();
	}

	function deactivateDropzones() {
		selectedElements.forEach((el) => el.classList.remove(DRAGGING_CLASS));
		if (clone) clone.remove();
		if (!opts) return;
		Array.from(opts.dropzoneRegistry.values()).forEach((dropzone) => {
			dropzone.deactivate();
		});

		if (timeoutId) {
			clearTimeout(timeoutId);
			timeoutId = undefined;
		}
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
			pushStatus: opts.pushStatus
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
			label: opts.label
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
			filePath: opts.filePath
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

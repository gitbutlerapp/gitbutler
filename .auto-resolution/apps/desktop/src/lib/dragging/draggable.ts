import { getColorFromCommitState } from '$components/lib';
import { type CommitStatusType } from '$lib/commits/commit';
import { ChangeDropData, type DropData } from '$lib/dragging/draggables';
import { getFileIcon } from '@gitbutler/ui/components/file/getFileIcon';
import iconsJson from '@gitbutler/ui/data/icons.json';
import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
import type { DropzoneRegistry } from '$lib/dragging/registry';
import type { DragStateService } from '@gitbutler/ui/drag/dragStateService.svelte';

// Added to element being dragged (not the clone that follows the cursor).
const DRAGGING_CLASS = 'dragging';

type chipType = 'file' | 'hunk' | 'ai-session';

export type DraggableConfig = {
	readonly selector?: string;
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
};

function createElement<K extends keyof HTMLElementTagNameMap>(
	tag: K,
	classNames: string[],
	textContent?: string,
	src?: string
): HTMLElementTagNameMap[K] {
	const el = document.createElement(tag);
	el.classList.add(...classNames);
	if (textContent) el.textContent = textContent;
	if (src) (el as HTMLImageElement).src = src;
	return el;
}

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

		if (opts.data instanceof ChangeDropData) {
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
//// COMMIT DRAGGABLE V3 ////
/////////////////////////////

function createCommitElementV3(
	commitType: CommitStatusType | undefined,
	label: string | undefined
): HTMLDivElement {
	const commitColor = getColorFromCommitState(commitType || 'LocalOnly', false);

	const cardEl = createElement('div', [
		'draggable-commit-v3',
		commitType === 'LocalOnly' || commitType === 'Integrated' || commitType === 'Base'
			? 'draggable-commit-v3-local'
			: 'draggable-commit-v3-remote'
	]);

	cardEl.style.setProperty('--commit-color', commitColor);

	const commitIndicationEl = createElement('div', ['draggable-commit-v3-indicator']);
	cardEl.appendChild(commitIndicationEl);

	const labelEl = createElement(
		'div',
		['truncate', 'text-13', 'text-semibold', 'draggable-commit-v3-label'],
		label || 'Empty commit'
	);
	cardEl.appendChild(labelEl);

	return cardEl;
}

export function draggableCommitV3(node: HTMLElement, initialOpts: DraggableConfig) {
	function createClone(opts: DraggableConfig) {
		if (opts.disabled) return;
		return createCommitElementV3(opts.commitType, opts.label);
	}
	return setupDragHandlers(node, initialOpts, createClone, {
		handlerWidth: false
	});
}

//////////////////
/// DRAG CHIPS ///
//////////////////

// --- CHIP CONTAINER HELPERS ---

function createFileChip(label?: string, filePath?: string): HTMLDivElement {
	const el = createElement('div', ['dragchip-file-container']);
	if (filePath) {
		const icon = getFileIcon(filePath);
		el.appendChild(createElement('img', ['dragchip-file-icon'], undefined, icon));
	}
	el.appendChild(
		createElement('span', ['text-12', 'text-semibold', 'dragchip-file-name'], label || 'Empty file')
	);
	return el;
}

function createHunkChip(label?: string): HTMLDivElement {
	const el = createElement('div', ['dragchip-hunk-container']);
	const deco = createElement('div', ['dragchip-hunk-decorator'], '〈/〉');
	el.appendChild(deco);
	el.appendChild(createElement('span', ['dragchip-hunk-label'], label || 'Empty hunk'));
	return el;
}

function createSVGIcon(pathData: string, classNames: string[]): SVGSVGElement {
	const icon = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
	icon.classList.add(...classNames);
	icon.setAttribute('viewBox', '0 0 16 16');
	icon.setAttribute('fill-rule', 'evenodd');
	const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
	path.setAttribute('fill', 'currentColor');
	path.setAttribute('d', pathData);
	icon.appendChild(path);
	return icon;
}

function createAISessionChip(label?: string): HTMLDivElement {
	const el = createElement('div', ['dragchip-ai-session-container']);
	const iconAi = createSVGIcon(iconsJson['ai-small'], ['dragchip-ai-session-icon']);
	el.appendChild(iconAi);
	if (label) {
		el.appendChild(
			createElement(
				'span',
				['text-12', 'text-semibold', 'truncate', 'dragchip-ai-session-label'],
				label
			)
		);
	}
	const iconDragHandle = createSVGIcon(iconsJson['draggable'], ['dragchip-ai-session-icon']);
	el.appendChild(iconDragHandle);
	return el;
}

export interface ChipsElementOptions {
	childrenAmount?: number;
	label?: string;
	filePath?: string;
	chipType?: chipType;
}

export function createChipsElement({
	childrenAmount = 1,
	label,
	filePath,
	chipType = 'file'
}: ChipsElementOptions = {}): HTMLDivElement {
	const container = createElement('div', ['dragchip-container']);

	if (chipType === 'ai-session') {
		container.appendChild(createAISessionChip(label));
	} else {
		const chip = createElement('div', ['dragchip']);
		container.appendChild(chip);

		if (chipType === 'file') {
			chip.appendChild(createFileChip(label, filePath));
		} else if (chipType === 'hunk') {
			chip.appendChild(createHunkChip(label));
		}

		if (childrenAmount > 1) {
			chip.appendChild(
				createElement('div', ['text-11', 'text-bold', 'dragchip-amount'], childrenAmount.toString())
			);
		}
	}

	if (childrenAmount === 2) {
		container.classList.add('dragchip-two');
	} else if (childrenAmount > 2) {
		container.classList.add('dragchip-multiple');
	}

	return container;
}

export function draggableChips(node: HTMLElement, initialOpts: DraggableConfig) {
	function createClone(opts: DraggableConfig, selectedElements: Element[]) {
		return createChipsElement({
			childrenAmount: selectedElements.length,
			label: opts.label,
			filePath: opts.filePath,
			chipType: opts.chipType || 'file'
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
	const ignoredElements = Array.from(cloneEl.querySelectorAll('[data-remove-from-draggable]'));
	ignoredElements.forEach((el) => el.remove());

	return cloneEl;
}

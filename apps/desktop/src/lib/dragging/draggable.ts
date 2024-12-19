import { dropzoneRegistry } from './dropzone';
import { type CommitStatus } from '$lib/vbranches/types';
import { getFileIcon } from '@gitbutler/ui/file/getFileIcon';
import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
import type { Draggable } from './draggables';
// Added to element being dragged (not the clone that follows the cursor).
const DRAGGING_CLASS = 'dragging';

export interface DraggableConfig {
	readonly selector?: string;
	readonly disabled?: boolean;
	readonly label?: string;
	readonly filePath?: string;
	readonly sha?: string;
	readonly date?: string;
	readonly authorImgUrl?: string;
	readonly commitType?: CommitStatus;
	readonly data?: Draggable | Promise<Draggable>;
	readonly viewportId?: string;
}

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
	opts: DraggableConfig,
	createClone: (opts: DraggableConfig, selectedElements: HTMLElement[]) => HTMLElement,
	params: {
		handlerWidth: boolean;
		maxHeight?: number;
	} = {
		handlerWidth: false
	}
) {
	let dragHandle: HTMLElement | null;
	let clone: HTMLElement;
	let selectedElements: HTMLElement[] = [];

	function handleMouseDown(e: MouseEvent) {
		dragHandle = e.target as HTMLElement;
	}

	function handleDragStart(e: DragEvent) {
		e.stopPropagation();

		if (dragHandle && dragHandle.dataset.noDrag !== undefined) {
			e.preventDefault();
			return false;
		}

		if (opts.selector) {
			const parentNode = node.parentElement?.parentElement;
			if (!parentNode) {
				console.error('draggable parent node not found');
				return;
			}
			selectedElements = Array.from(
				parentNode.querySelectorAll(opts.selector) as NodeListOf<HTMLElement>
			);
		}

		if (selectedElements.length === 0) {
			selectedElements = [node];
		}

		clone = createClone(opts, selectedElements);
		if (params.handlerWidth) {
			clone.style.width = node.clientWidth + 'px';
		}
		if (params.maxHeight) {
			clone.style.maxHeight = pxToRem(params.maxHeight) as string;
		}

		selectedElements.forEach((el) => el.classList.add(DRAGGING_CLASS));
		document.body.appendChild(clone);

		Array.from(dropzoneRegistry.values()).forEach((dropzone) => {
			dropzone.register(opts.data);
		});

		if (e.dataTransfer) {
			if (params.handlerWidth) {
				e.dataTransfer.setDragImage(clone, e.offsetX, e.offsetY);
			} else {
				e.dataTransfer.setDragImage(clone, clone.offsetWidth - 20, 25);
			}

			// Get chromium to fire dragover & drop events
			// https://stackoverflow.com/questions/6481094/html5-drag-and-drop-ondragover-not-firing-in-chrome/6483205#6483205
			e.dataTransfer?.setData('text/html', 'd'); // cannot be empty string
			e.dataTransfer.effectAllowed = 'uninitialized';
		}
	}

	const viewport = opts.viewportId ? document.getElementById(opts.viewportId) : null;
	const triggerRange = 150;
	const timerShutter = 700;
	let timeoutId: undefined | ReturnType<typeof setTimeout> = undefined;

	function loopScroll(viewport: HTMLElement, direction: 'left' | 'right', scrollSpeed: number) {
		viewport.scrollBy({
			left: direction === 'left' ? -scrollSpeed : scrollSpeed,
			// left: direction === 'left' ? -40 : 40,
			behavior: 'smooth'
		});

		timeoutId = setTimeout(() => loopScroll(viewport, direction, scrollSpeed), timerShutter); // Store the timeout ID
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
		e.stopPropagation();
		if (clone) clone.remove();
		selectedElements.forEach((el) => el.classList.remove(DRAGGING_CLASS));
		Array.from(dropzoneRegistry.values()).forEach((dropzone) => {
			dropzone.unregister();
		});

		if (timeoutId) {
			// Clear the timeout
			clearTimeout(timeoutId);
			timeoutId = undefined;
		}
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
		node.draggable = false;
		node.removeEventListener('dragstart', handleDragStart);
		node.removeEventListener('drag', handleDrag);
		node.removeEventListener('dragend', handleDragEnd);
		node.removeEventListener('mousedown', handleMouseDown, { capture: false });
	}

	setup(opts);

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

//////////////////////////
//// COMMIT DRAGGABLE ////
//////////////////////////

export function createCommitElement(
	commitType: CommitStatus | undefined,
	label: string | undefined,
	sha: string | undefined,
	date: string | undefined,
	authorImgUrl: string | undefined
): HTMLDivElement {
	const cardEl = createElement('div', ['draggable-commit']);
	const labelEl = createElement('span', ['text-13', 'text-bold'], label || 'Empty commit');
	const infoEl = createElement('div', ['draggable-commit-info', 'text-11']);
	const authorImgEl = createElement(
		'img',
		['draggable-commit-author-img'],
		undefined,
		authorImgUrl
	);
	const shaEl = createElement('span', ['draggable-commit-info-text'], sha);
	const dateAndAuthorEl = createElement('span', ['draggable-commit-info-text'], date);

	if (commitType) {
		const indicatorClass = `draggable-commit-${commitType}`;
		labelEl.classList.add('draggable-commit-indicator', indicatorClass);
	}

	cardEl.appendChild(labelEl);
	infoEl.appendChild(authorImgEl);
	infoEl.appendChild(shaEl);
	infoEl.appendChild(dateAndAuthorEl);
	cardEl.appendChild(infoEl);
	return cardEl;
}

export function draggableCommit(node: HTMLElement, initialOpts: DraggableConfig) {
	function createClone(opts: DraggableConfig) {
		return createCommitElement(opts.commitType, opts.label, opts.sha, opts.date, opts.authorImgUrl);
	}
	return setupDragHandlers(node, initialOpts, createClone, {
		handlerWidth: true
	});
}

////////////////////////
//// FILE DRAGGABLE ////
////////////////////////

export function createChipsElement(
	childrenAmount: number,
	label: string | undefined,
	filePath: string | undefined
): HTMLDivElement {
	const containerEl = createElement('div', ['draggable-chip-container']);
	const chipEl = createElement('div', ['draggable-chip']);
	containerEl.appendChild(chipEl);

	if (filePath) {
		const iconEl = createElement('img', ['draggable-chip-icon'], undefined, getFileIcon(filePath));
		chipEl.appendChild(iconEl);
	}

	const labelEl = createElement('span', ['text-12'], label);
	chipEl.appendChild(labelEl);

	if (childrenAmount > 1) {
		const amountTag = createElement(
			'div',
			['text-11', 'text-bold', 'draggable-chip-amount'],
			childrenAmount.toString()
		);
		chipEl.appendChild(amountTag);
	}

	if (childrenAmount === 2) {
		containerEl.classList.add('draggable-chip-two');
	} else if (childrenAmount > 2) {
		containerEl.classList.add('draggable-chip-multiple');
	}

	return containerEl;
}

export function draggableChips(node: HTMLElement, initialOpts: DraggableConfig) {
	function createClone(opts: DraggableConfig, selectedElements: HTMLElement[]) {
		return createChipsElement(selectedElements.length, opts.label, opts.filePath);
	}
	return setupDragHandlers(node, initialOpts, createClone);
}

////////////////////////
//// HUNK DRAGGABLE ////
////////////////////////

export function cloneElement(node: HTMLElement) {
	const cloneEl = node.cloneNode(true) as HTMLElement;

	// exclude all ignored elements from the clone
	const ignoredElements = Array.from(cloneEl.querySelectorAll('[data-remove-from-draggable]'));
	ignoredElements.forEach((el) => el.remove());

	return cloneEl;
}

export function draggableElement(node: HTMLElement, initialOpts: DraggableConfig) {
	function createClone() {
		return cloneElement(node);
	}
	return setupDragHandlers(node, initialOpts, createClone, {
		handlerWidth: true
	});
}

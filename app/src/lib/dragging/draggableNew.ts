import { dropzoneRegistry } from './dropzone';
import { getVSIFileIcon } from '$lib/ext-icons';
import { type CommitStatus } from '$lib/vbranches/types';
import type { Draggable } from './draggables';

export interface DraggableConfig {
	readonly selector?: string;
	readonly disabled?: boolean;
	readonly label?: string;
	readonly filePath?: string;
	readonly commitType?: CommitStatus;
	readonly data?: Draggable | Promise<Draggable>;
	readonly viewportId?: string;
}

function createElement(
	tag: string,
	classNames: string[],
	textContent?: string,
	src?: string
): HTMLElement {
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
	handlerWidth: boolean = false
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
			selectedElements = selectedElements.length ? selectedElements : [node];
		}

		clone = createClone(opts, selectedElements);
		if (handlerWidth) {
			clone.style.width = node.clientWidth + 'px';
		}
		selectedElements.forEach((el) => (el.style.opacity = '0.5'));
		document.body.appendChild(clone);

		Array.from(dropzoneRegistry.values()).forEach((dropzone) => {
			dropzone.register(opts.data);
		});

		if (e.dataTransfer) {
			e.dataTransfer.setDragImage(clone, e.offsetX, e.offsetY);
			e.dataTransfer.effectAllowed = 'uninitialized';
		}
	}

	function handleDragEnd(e: DragEvent) {
		e.stopPropagation();
		if (clone) clone.remove();
		selectedElements.forEach((el) => (el.style.opacity = '1'));
		Array.from(dropzoneRegistry.values()).forEach((dropzone) => {
			dropzone.unregister();
		});
	}

	function handleDrag(e: DragEvent) {
		e.preventDefault();
		const viewport = opts.viewportId ? document.getElementById(opts.viewportId) : null;
		if (!viewport) return;
		const triggerRange = 150;
		const scrollSpeed = (viewport.clientWidth || 500) / 2;
		const viewportWidth = viewport.clientWidth;
		const relativeX = e.clientX - viewport.getBoundingClientRect().left;
		if (relativeX < triggerRange) {
			viewport.scrollBy(-scrollSpeed, 0);
		} else if (relativeX > viewportWidth - triggerRange) {
			viewport.scrollBy(scrollSpeed, 0);
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

// COMMIT DRAGGABLE

export function createCommitElement(
	commitType: CommitStatus | undefined,
	label: string | undefined
): HTMLDivElement {
	const chipEl = createElement('div', ['draggable-commit']) as HTMLDivElement;
	const labelEl = createElement('span', ['text-base-13', 'text-bold'], label);

	if (commitType) {
		const indicatorClass = `draggable-commit-${commitType}`;
		labelEl.classList.add('draggable-commit-indicator', indicatorClass);
	}

	chipEl.appendChild(labelEl);
	return chipEl;
}

export function draggableCommit(node: HTMLElement, initialOpts: DraggableConfig) {
	function createClone(opts: DraggableConfig) {
		return createCommitElement(opts.commitType, opts.label);
	}
	return setupDragHandlers(node, initialOpts, createClone, true);
}

// FILE DRAGGABLE

export function createChipsElement(
	childrenAmount: number,
	label: string | undefined,
	filePath: string | undefined
): HTMLDivElement {
	const containerEl = createElement('div', ['draggable-chip-container']) as HTMLDivElement;
	const chipEl = createElement('div', ['draggable-chip']);
	containerEl.appendChild(chipEl);

	if (filePath) {
		const iconEl = createElement(
			'img',
			['draggable-chip-icon'],
			undefined,
			getVSIFileIcon(filePath)
		);
		chipEl.appendChild(iconEl);
	}

	const labelEl = createElement('span', ['text-base-12'], label);
	chipEl.appendChild(labelEl);

	if (childrenAmount > 1) {
		const amountTag = createElement(
			'div',
			['text-base-11', 'text-bold', 'draggable-chip-amount'],
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

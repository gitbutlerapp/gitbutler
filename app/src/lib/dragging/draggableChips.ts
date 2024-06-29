import { dropzoneRegistry } from './dropzone';
import { getVSIFileIcon } from '$lib/ext-icons';
import type { Draggable } from './draggables';

export type DraggableItemKind = 'file' | 'commit' | undefined;

export interface DraggableConfig {
	readonly selector?: string;
	readonly disabled?: boolean;
	readonly kind?: DraggableItemKind;
	readonly label?: string;
	readonly filePath?: string;
	readonly data?: Draggable | Promise<Draggable>;
	readonly viewportId?: string;
}

export function createChipsElement(
	childrenAmount: number,
	label: string | undefined,
	kind: DraggableItemKind,
	filePath: string | undefined
): HTMLDivElement {
	// CREATE CONTAINER
	const containerEl = document.createElement('div');
	containerEl.classList.add('draggable-chip-container');

	// CREATE CHIP
	const chipEl = document.createElement('div');
	chipEl.classList.add('draggable-chip');
	containerEl.appendChild(chipEl);

	// CREATE ICON
	if (kind === 'file' && filePath) {
		const iconEl = document.createElement('img');
		iconEl.classList.add('draggable-chip-icon');
		iconEl.src = getVSIFileIcon(filePath);
		chipEl.appendChild(iconEl);
	}

	// CREATE LABEL
	const labelEl = document.createElement('span');
	labelEl.classList.add('text-base-12');
	labelEl.textContent = label || '';
	chipEl.appendChild(labelEl);

	// CREATE AMOUNT TAG
	if (childrenAmount > 1) {
		const amountTag = document.createElement('div');
		amountTag.classList.add('text-base-11', 'text-bold', 'draggable-chip-amount');
		amountTag.textContent = childrenAmount.toString();
		chipEl.appendChild(amountTag);
	}

	if (childrenAmount === 2) {
		containerEl.classList.add('draggable-chip-two');
	}

	if (childrenAmount > 2) {
		containerEl.classList.add('draggable-chip-multiple');
	}

	return containerEl;
}

export function draggable(node: HTMLElement, initialOpts: DraggableConfig) {
	let opts = initialOpts;
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
			// Checking for selected siblings in the parent of the parent container likely works
			// for most use-cases but it was done here primarily for dragging multiple files.
			const parentNode = node.parentNode?.parentNode;

			if (!parentNode) {
				console.error('draggable parent node not found');
				return;
			}

			selectedElements = Array.from(
				parentNode.querySelectorAll(opts.selector).values() as Iterable<HTMLElement>
			);
			selectedElements = selectedElements.length > 0 ? selectedElements : [node];

			clone = createChipsElement(selectedElements.length, opts.label, opts.kind, opts.filePath);

			// Dim the original element while dragging
			selectedElements.forEach((element) => {
				element.style.opacity = '0.5';
			});
		}

		document.body.appendChild(clone);

		Array.from(dropzoneRegistry.values()).forEach((dropzone) => {
			dropzone.register(opts.data);
		});

		// Get chromium to fire dragover & drop events
		// https://stackoverflow.com/questions/6481094/html5-drag-and-drop-ondragover-not-firing-in-chrome/6483205#6483205
		// e.dataTransfer?.setData('text/html', 'placeholder copy'); // cannot be empty string

		if (e.dataTransfer) {
			e.dataTransfer.setDragImage(clone, clone.offsetWidth - 30, 25); // Adds the padding
			e.dataTransfer.effectAllowed = 'uninitialized';
		}
	}

	function handleDragEnd(e: DragEvent) {
		e.stopPropagation();

		if (clone) {
			clone.remove();
		}

		// reset the opacity of the selected elements
		selectedElements.forEach((element) => {
			element.style.opacity = '1';
		});

		Array.from(dropzoneRegistry.values()).forEach((dropzone) => {
			dropzone.unregister();
		});
	}

	const viewport = opts.viewportId ? document.getElementById(opts.viewportId) : null;
	const triggerRange = 150;
	const scrollSpeed = (viewport?.clientWidth || 500) / 2;
	let lastDrag = new Date().getTime();

	function handleDrag(e: DragEvent) {
		e.preventDefault();

		if (!viewport) return;
		if (new Date().getTime() - lastDrag < 500) return;
		lastDrag = new Date().getTime();

		const viewportWidth = viewport.clientWidth;
		const relativeX = e.clientX - viewport.getBoundingClientRect().left;

		// Scroll horizontally if the draggable is near the edge of the viewport
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
		update(opts: DraggableConfig) {
			clean();
			setup(opts);
		},
		destroy() {
			clean();
		}
	};
}

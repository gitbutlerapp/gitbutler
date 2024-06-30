import { dropzoneRegistry } from './dropzone';
import { type CommitStatus } from '$lib/vbranches/types';
import type { Draggable } from './draggables';

export interface DraggableConfig {
	readonly selector?: string;
	readonly disabled?: boolean;
	readonly label?: string;
	readonly commitType?: CommitStatus;
	readonly data?: Draggable | Promise<Draggable>;
	readonly viewportId?: string;
}

export function createChipsElement(
	commitType: CommitStatus | undefined,
	label: string | undefined
): HTMLDivElement {
	// CREATE CHIP
	const chipEl = document.createElement('div');
	chipEl.classList.add('draggable-commmit');

	// CREATE LABEL
	const labelEl = document.createElement('span');
	labelEl.classList.add('text-base-13', 'text-bold');

	labelEl.textContent = label || '';
	chipEl.appendChild(labelEl);

	if (commitType === 'localAndRemote') {
		labelEl.classList.add('draggable-commmit-indicator', 'draggable-commmit-remote');
	}
	if (commitType === 'local') {
		labelEl.classList.add('draggable-commmit-indicator', 'draggable-commmit-local');
	}

	return chipEl;
}

export function draggable(node: HTMLElement, initialOpts: DraggableConfig) {
	let opts = initialOpts;
	let dragHandle: HTMLElement | null;
	let clone: HTMLElement;

	function handleMouseDown(e: MouseEvent) {
		dragHandle = e.target as HTMLElement;
	}

	function handleDragStart(e: DragEvent) {
		e.stopPropagation();

		if (dragHandle && dragHandle.dataset.noDrag !== undefined) {
			e.preventDefault();
			return false;
		}

		node.style.opacity = '0.5';

		clone = createChipsElement(opts.commitType, opts.label);
		clone.style.width = node.clientWidth + 'px';

		document.body.appendChild(clone);

		Array.from(dropzoneRegistry.values()).forEach((dropzone) => {
			dropzone.register(opts.data);
		});

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
		node.style.opacity = '1';

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

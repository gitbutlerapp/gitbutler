import { dzRegistry } from './dropzone';
import type { DraggableCommit, DraggableFile, DraggableHunk } from './draggables';

export interface DraggableConfig {
	readonly selector?: string;
	readonly disabled?: boolean;
	readonly data?: DraggableFile | DraggableHunk | DraggableCommit;
	readonly viewportId?: string;
}

export function applyContainerStyle(element: HTMLElement) {
	element.style.position = 'absolute';
	element.style.top = '-9999px'; // Element has to be in the DOM so we move it out of sight
	element.style.display = 'inline-block';
	element.style.padding = '30px'; // To prevent clipping of rotated element
}

export function createContainerForMultiDrag(
	children: Element[],
	containerWidth: number
): HTMLDivElement {
	const inner = document.createElement('div');
	inner.style.display = 'flex';
	inner.style.flexDirection = 'column';
	inner.style.gap = 'var(--size-2)';

	// need to set the width in order to make all the children have the same width
	// this is necessary for the tree view where the children are nested and have different widths
	inner.style.width = containerWidth + 'px';

	children.forEach((child) => {
		inner.appendChild(cloneWithPreservedDimensions(child));
	});
	rotateElement(inner);

	const container = document.createElement('div');
	container.appendChild(inner);
	applyContainerStyle(container);

	return container;
}

export function cloneWithPreservedDimensions(node: any) {
	const clone = node.cloneNode(true) as HTMLElement;
	clone.style.height = node.clientHeight + 'px';
	clone.classList.remove('selected-draggable');
	return clone;
}

export function cloneWithRotation(node: any) {
	const container = document.createElement('div');
	const clone = cloneWithPreservedDimensions(node) as HTMLElement;
	container.appendChild(clone);

	// exclude all ignored elements from the clone
	const ignoredElements = container.querySelectorAll('[data-remove-from-draggable]');
	ignoredElements.forEach((element) => {
		element.remove();
	});

	applyContainerStyle(container);

	// Style the inner node so it retains the shape and then rotate
	// TODO: This rotation puts a requirement on draggables to have
	// an outer container, which feels extra. Consider refactoring.
	rotateElement(clone);
	return container as HTMLElement;
}

function rotateElement(element: HTMLElement) {
	element.style.rotate = `${Math.floor(Math.random() * 3)}deg`;
}

export function draggable(node: HTMLElement, opts: DraggableConfig) {
	let dragHandle: HTMLElement | null;
	let clone: HTMLElement | undefined;

	let selectedElements: HTMLElement[] = [];

	const onDropListeners = new Map<HTMLElement, Array<(e: DragEvent) => void>>();
	const onDragLeaveListeners = new Map<HTMLElement, Array<(e: DragEvent) => void>>();
	const onDragEnterListeners = new Map<HTMLElement, Array<(e: DragEvent) => void>>();
	const onDragOverListeners = new Map<HTMLElement, Array<(e: DragEvent) => void>>();

	function handleMouseDown(e: MouseEvent) {
		dragHandle = e.target as HTMLElement;
	}

	function handleDragStart(e: DragEvent) {
		let elt: HTMLElement | null = dragHandle;
		while (elt) {
			if (elt.dataset.noDrag !== undefined) {
				e.stopPropagation();
				e.preventDefault();
				return false;
			}
			elt = elt.parentElement;
		}

		// If the draggable specifies a selector then we check if we're dragging selected
		// elements, falling back to the single node executing the drag.
		if (opts.selector) {
			selectedElements = Array.from(
				document.querySelectorAll(opts.selector).values() as Iterable<HTMLElement>
			);

			if (selectedElements.length > 0) {
				const firstChildWidth = selectedElements[0].clientWidth;
				clone = createContainerForMultiDrag(selectedElements, firstChildWidth);

				// Dim the original element while dragging
				selectedElements.forEach((element) => {
					element.style.opacity = '0.5';
				});
			}
		}
		if (!clone) {
			clone = cloneWithRotation(node);
		}

		document.body.appendChild(clone);

		// activate destination zones
		dzRegistry
			.filter(([_node, dz]) => dz.accepts(opts.data))
			.forEach(([target, dz]) => {
				function onDrop(e: DragEvent) {
					e.preventDefault();
					dz.onDrop(opts.data);
				}

				function onDragEnter(e: DragEvent) {
					e.preventDefault();
					target.classList.add(dz.hover);
				}

				function onDragLeave(e: DragEvent) {
					e.preventDefault();
					target.classList.remove(dz.hover);
				}

				function onDragOver(e: DragEvent) {
					e.preventDefault();
				}

				// keep track of listeners so that we can remove them later
				if (onDropListeners.has(target)) {
					onDropListeners.get(target)!.push(onDrop);
				} else {
					onDropListeners.set(target, [onDrop]);
				}

				if (onDragEnterListeners.has(target)) {
					onDragEnterListeners.get(target)!.push(onDragEnter);
				} else {
					onDragEnterListeners.set(target, [onDragEnter]);
				}

				if (onDragLeaveListeners.has(target)) {
					onDragLeaveListeners.get(target)!.push(onDragLeave);
				} else {
					onDragLeaveListeners.set(target, [onDragLeave]);
				}

				if (onDragOverListeners.has(target)) {
					onDragOverListeners.get(target)!.push(onDragOver);
				} else {
					onDragOverListeners.set(target, [onDragOver]);
				}

				// https://stackoverflow.com/questions/14203734/dragend-dragenter-and-dragleave-firing-off-immediately-when-i-drag
				setTimeout(() => {
					target.classList.add(dz.active);
				}, 10);

				target.addEventListener('drop', onDrop);
				target.addEventListener('dragenter', onDragEnter);
				target.addEventListener('dragleave', onDragLeave);
				target.addEventListener('dragover', onDragOver);
			});

		// Get chromium to fire dragover & drop events
		// https://stackoverflow.com/questions/6481094/html5-drag-and-drop-ondragover-not-firing-in-chrome/6483205#6483205
		e.dataTransfer?.setData('text/html', 'd'); // cannot be empty string
		e.dataTransfer?.setDragImage(clone, e.offsetX + 30, e.offsetY + 30); // Adds the padding
		e.stopPropagation();
	}

	function handleDragEnd(e: DragEvent) {
		if (clone) {
			clone.remove();
			clone = undefined;
		}

		// reset the opacity of the selected elements
		selectedElements.forEach((element) => {
			element.style.opacity = '1';
		});

		// deactivate destination zones
		dzRegistry
			.filter(([_node, dz]) => dz.accepts(opts.data))
			.forEach(([node, dz]) => {
				// remove all listeners
				onDropListeners.get(node)?.forEach((listener) => {
					node.removeEventListener('drop', listener);
				});
				onDragEnterListeners.get(node)?.forEach((listener) => {
					node.removeEventListener('dragenter', listener);
				});
				onDragLeaveListeners.get(node)?.forEach((listener) => {
					node.removeEventListener('dragleave', listener);
				});
				onDragOverListeners.get(node)?.forEach((listener) => {
					node.removeEventListener('dragover', listener);
				});

				node.classList.remove(dz.active);
				node.classList.remove(dz.hover);
			});

		e.stopPropagation();
	}

	const viewport = opts.viewportId ? document.getElementById(opts.viewportId) : null;
	const triggerRange = 100;
	const scrollSpeed = 5;

	function handleDrag(e: DragEvent) {
		if (!viewport) return;

		const viewportWidth = viewport.clientWidth;
		const relativeX = e.clientX - viewport.getBoundingClientRect().left;

		// Scroll horizontally if the draggable is near the edge of the viewport
		if (relativeX < triggerRange) {
			viewport.scrollBy(-scrollSpeed, 0);
		} else if (relativeX > viewportWidth - triggerRange) {
			viewport.scrollBy(scrollSpeed, 0);
		}
	}

	function setup(opts: DraggableConfig) {
		if (opts.disabled) return;
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

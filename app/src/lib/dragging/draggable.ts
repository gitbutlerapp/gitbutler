import { dzRegistry } from './dropzone';
import type { Draggable } from './draggables';

export interface DraggableConfig {
	readonly selector?: string;
	readonly disabled?: boolean;
	readonly data?: Draggable | Promise<Draggable>;
	readonly viewportId?: string;
	readonly extendWithClass?: string;
}

export function applyContainerStyle(element: HTMLElement) {
	element.style.position = 'absolute';
	element.style.top = '-9999px'; // Element has to be in the DOM so we move it out of sight
	element.style.display = 'inline-block';
	element.style.padding = '30px'; // To prevent clipping of rotated element
}

export function createContainerForMultiDrag(
	children: Element[],
	extendWithClass: string | undefined
): HTMLDivElement {
	const inner = document.createElement('div');
	inner.style.display = 'flex';
	inner.style.flexDirection = 'column';
	inner.style.gap = '0.125rem';

	children.forEach((child) => {
		inner.appendChild(cloneWithPreservedDimensions(child, extendWithClass));
	});
	rotateElement(inner);

	const container = document.createElement('div');
	container.appendChild(inner);
	applyContainerStyle(container);

	return container;
}

export function cloneWithPreservedDimensions(node: any, extendWithClass: string | undefined) {
	const clone = node.cloneNode(true) as HTMLElement;
	clone.style.height = node.clientHeight + 'px';
	clone.style.width = node.clientWidth + 'px';
	clone.classList.remove('selected-draggable');

	extendWithClass && clone.classList.add(extendWithClass);

	return clone;
}

export function cloneWithRotation(node: any, extendWithClass: string | undefined = undefined) {
	const container = document.createElement('div');
	const clone = cloneWithPreservedDimensions(node, extendWithClass) as HTMLElement;
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

export function draggable(node: HTMLElement, initialOpts: DraggableConfig) {
	let opts = initialOpts;
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

		// If the draggable specifies a selector then we check if we're dragging selected elements
		if (opts.selector) {
			// Checking for selected siblings in the parent of the parent container likely works
			// for most use-cases but it was done here primarily for dragging multiple files.
			const parentNode = node.parentNode?.parentNode;
			selectedElements = parentNode
				? Array.from(parentNode.querySelectorAll(opts.selector).values() as Iterable<HTMLElement>)
				: [];

			if (selectedElements.length > 0) {
				clone = createContainerForMultiDrag(selectedElements, opts.extendWithClass);
				// Dim the original element while dragging
				selectedElements.forEach((element) => {
					element.style.opacity = '0.5';
				});
			}
		}

		if (!clone) {
			clone = cloneWithRotation(node, opts.extendWithClass);
		}

		document.body.appendChild(clone);

		// activate destination zones
		dzRegistry.forEach(async ([target, dz]) => {
			if (!dz.accepts(await opts.data)) return;

			async function onDrop(e: DragEvent) {
				e.preventDefault();
				dz.onDrop(await opts.data);
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
		dzRegistry.forEach(async ([node, dz]) => {
			if (!dz.accepts(await opts.data)) return;
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
	const triggerRange = 150;
	const scrollSpeed = (viewport?.clientWidth || 500) / 2;
	let lastDrag = new Date().getTime();

	function handleDrag(e: DragEvent) {
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

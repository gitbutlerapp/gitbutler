export interface Dropzone {
	disabled: boolean;
	active: string;
	hover: string;
	accepts: (data: any) => boolean;
	onDrop: (data: any) => Promise<void> | void;
}

const defaultDropzoneOptions: Dropzone = {
	disabled: false,
	active: 'dropzone-active',
	hover: 'dropzone-hover',
	accepts: (data) => data == 'default',
	onDrop: () => {}
};

export function dropzone(node: HTMLElement, opts: Partial<Dropzone> | undefined) {
	let currentOptions = { ...defaultDropzoneOptions, ...opts };

	function handleDragEnter(e: DragEvent) {
		if (activeZones.has(node)) {
			node.classList.add(currentOptions.hover);
			e.preventDefault();
		}
	}

	function handleDragLeave(_e: DragEvent) {
		if (activeZones.has(node)) {
			node.classList.remove(currentOptions.hover);
		}
	}

	function handleDragOver(e: DragEvent) {
		if (activeZones.has(node)) {
			e.preventDefault();
		}
	}

	function setup(opts: Partial<Dropzone> | undefined) {
		currentOptions = { ...defaultDropzoneOptions, ...opts };
		if (currentOptions.disabled) return;

		register(node, currentOptions);

		node.addEventListener('dragenter', handleDragEnter);
		node.addEventListener('dragleave', handleDragLeave);
		node.addEventListener('dragover', handleDragOver);
	}

	function clean() {
		unregister(currentOptions);

		node.removeEventListener('dragenter', handleDragEnter);
		node.removeEventListener('dragleave', handleDragLeave);
		node.removeEventListener('dragover', handleDragOver);
	}

	setup(opts);

	return {
		update(opts: Partial<Dropzone> | undefined) {
			clean();
			setup(opts);
		},
		destroy() {
			clean();
		}
	};
}

const registry: [HTMLElement, Dropzone][] = [];

const activeZones = new Set<HTMLElement>();

function register(node: HTMLElement, dropzone: Dropzone) {
	registry.push([node, dropzone]);
}

function unregister(dropzone: Dropzone) {
	const index = registry.findIndex(([, dz]) => dz == dropzone);
	if (index >= 0) registry.splice(index, 1);
}

export interface Draggable {
	data: any;
	disabled: boolean;
}

const defaultDraggableOptions: Draggable = {
	data: 'default',
	disabled: false
};

export function draggable(node: HTMLElement, opts: Partial<Draggable> | undefined) {
	let currentOptions = { ...defaultDraggableOptions, ...opts };
	let clone: HTMLElement;

	const onDropListeners = new Map<HTMLElement, Array<(e: DragEvent) => void>>();

	/**
	 * The problem with the ghost element is that it gets clipped after rotation unless we enclose
	 * it within a larger bounding box. This means we have an extra `<div>` in the html that is
	 * only present to support the rotation
	 */
	function handleDragStart(e: DragEvent) {
		// Start by cloning the node for the ghost element
		clone = node.cloneNode(true) as HTMLElement;
		clone.style.position = 'absolute';
		clone.style.top = '-9999px'; // Element has to be in the DOM so we move it out of sight
		clone.style.display = 'inline-block';
		clone.style.padding = '30px'; // To prevent clipping of rotated element

		// Style the inner node so it retains the shape and then rotate
		const inner = clone.children[0] as HTMLElement;
		inner.style.height = node.clientHeight + 'px';
		inner.style.width = node.clientWidth + 'px';
		inner.style.rotate = `${Math.floor(Math.random() * 3)}deg`;
		document.body.appendChild(clone);

		// Dim the original element while dragging
		node.style.opacity = '0.6';

		// activate destination zones
		registry
			.filter(([_node, dz]) => dz.accepts(currentOptions.data))
			.forEach(([target, dz]) => {
				const onDrop = (e: DragEvent) => {
					e.preventDefault();
					dz.onDrop(currentOptions.data);
				};

				// keep track of listeners so that we can remove them later
				if (onDropListeners.has(target)) {
					onDropListeners.get(target)!.push(onDrop);
				} else {
					onDropListeners.set(target, [onDrop]);
				}

				// https://stackoverflow.com/questions/14203734/dragend-dragenter-and-dragleave-firing-off-immediately-when-i-drag
				setTimeout(() => target.classList.add(dz.active), 10);

				target.addEventListener('drop', onDrop);
				activeZones.add(target);
			});

		e.dataTransfer?.setDragImage(clone, e.offsetX + 30, e.offsetY + 30); // Adds the padding
		e.stopPropagation();
	}

	function handleDragEnd(e: DragEvent) {
		node.style.opacity = '1';
		clone.remove();

		// deactivate destination zones
		registry
			.filter(([_node, dz]) => dz.accepts(currentOptions.data))
			.forEach(([node, dz]) => {
				// remove all listeners
				const onDrop = onDropListeners.get(node);
				if (onDrop) {
					onDrop.forEach((listener) => {
						node.removeEventListener('drop', listener);
					});
				}

				node.classList.remove(dz.active);
				activeZones.delete(node);
			});

		e.stopPropagation();
	}

	function setup(opts: Partial<Draggable> | undefined) {
		currentOptions = { ...defaultDraggableOptions, ...opts };

		if (currentOptions.disabled) return;

		node.draggable = true;

		node.addEventListener('dragstart', handleDragStart);
		node.addEventListener('dragend', handleDragEnd);
	}

	function clean() {
		node.draggable = false;
		node.removeEventListener('dragstart', handleDragStart);
		node.removeEventListener('dragend', handleDragEnd);
	}

	setup(opts);

	return {
		update(opts: Partial<Draggable> | undefined) {
			clean();
			setup(opts);
		},
		destroy() {
			clean();
		}
	};
}

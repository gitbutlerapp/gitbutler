const zoneMap = new Map<string, Set<HTMLElement>>();
const hightlightOptionsMap = new Map<HTMLElement, HighlightOptions>();

function inactivateZones(types: string[]) {
	types.forEach((type) => {
		getZones(type).forEach((zone) => {
			const opts = hightlightOptionsMap.get(zone);
			if (opts && Object.keys(opts.handlers).includes(type)) {
				zone.classList.remove(opts.active);
			}
		});
	});
}

function activateZones(trigger: HTMLElement, types: string[]) {
	types.forEach((type) => {
		getZones(type).forEach((zone) => {
			if (zone === trigger) return;
			if (isChildOf(trigger, zone)) return;

			const opts = hightlightOptionsMap.get(zone);
			if (opts && Object.keys(opts.handlers).includes(type)) {
				zone.classList.add(opts.active);
			}
		});
	});
}

function getZones(type: string): Set<HTMLElement> {
	let zones = zoneMap.get(type);
	if (!zones) {
		zones = new Set([]);
		zoneMap.set(type, zones);
	}
	return zones;
}

function isChildOf(child: any, parent: HTMLElement): boolean {
	if (parent === child) return false;
	if (!child.parentElement) return false;
	if (child.parentElement == parent) return true;
	return isChildOf(child.parentElement, parent);
}

export type TriggerOptions = {
	disabled: boolean;
	data: Record<string, any>;
};

const defaultTriggerOptions: TriggerOptions = {
	disabled: false,
	data: { default: {} }
};

export function dzTrigger(node: HTMLElement, opts: Partial<TriggerOptions> | undefined) {
	const options = { ...defaultTriggerOptions, ...opts };
	if (options.disabled) return;

	node.draggable = true;

	let clone: HTMLElement;

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

		e.dataTransfer?.setDragImage(clone, e.offsetX + 30, e.offsetY + 30); // Adds the padding

		Object.entries(options.data).forEach(([type, data]) => {
			e.dataTransfer?.setData(type, data);
		});

		activateZones(node, Object.keys(options.data));

		e.stopPropagation();
	}

	function handleDragEnd(e: DragEvent) {
		node.style.opacity = '1'; // Undo the dimming from `dragstart`
		clone.remove(); // Remove temporary ghost element

		inactivateZones(Object.keys(options.data));
		e.stopPropagation();
	}

	node.draggable = true;
	node.addEventListener('dragstart', handleDragStart);
	node.addEventListener('dragend', handleDragEnd);

	return {
		destroy() {
			node.removeEventListener('dragstart', handleDragStart);
			node.removeEventListener('dragend', handleDragEnd);
		}
	};
}

export interface HighlightOptions {
	handlers: Record<string, (data: any) => void>;
	hover: string;
	active: string;
}

const defaultHighlightOptions: HighlightOptions = {
	hover: 'drop-zone-hover',
	active: 'drop-zone-active',
	handlers: { default: () => {} }
};

export function dzHighlight(node: HTMLElement, opts: Partial<HighlightOptions> | undefined) {
	const options = { ...defaultHighlightOptions, ...opts };

	// register this node as a drop target for each of the handler types
	Object.keys(options.handlers).forEach((type) => {
		const zones = getZones(type);
		zones.add(node);
		hightlightOptionsMap.set(node, options);
	});

	function setHover(value: boolean) {
		if (value) {
			// We do this so we can set pointer-events-none on all dropzones from main css file,
			// without it onMouseLeave fires every time a child container is left.
			node.classList.add(defaultHighlightOptions.hover);
			node.classList.add(options.hover);
		} else {
			node.classList.remove(defaultHighlightOptions.hover);
			node.classList.remove(options.hover);
		}
	}

	function isValidEvent(e: DragEvent): boolean {
		const validTypes = Object.keys(options.handlers);
		const eventTypes = e.dataTransfer?.types || [];
		return eventTypes.some((type) => validTypes.includes(type));
	}

	function handleDragEnter(e: DragEvent) {
		if (!isValidEvent(e)) {
			return;
		}

		setHover(true);
		e.stopPropagation();
	}

	function handleDragLeave(e: DragEvent) {
		if (!isValidEvent(e)) {
			return;
		}

		if (!isChildOf(e.target, node)) {
			setHover(false);
		}

		e.stopPropagation();
	}

	function handleDragEnd(e: DragEvent) {
		if (!isValidEvent(e)) {
			return;
		}

		setHover(false);
		e.stopPropagation();
	}

	function handleDrop(e: DragEvent) {
		if (!isValidEvent(e)) {
			return;
		}
		setHover(false);
	}

	function handleDragOver(e: DragEvent) {
		if (!isValidEvent(e)) {
			e.stopImmediatePropagation(); // Stops event from reaching `on:dragover` on the element
			return;
		}

		e.preventDefault();
	}

	node.addEventListener('dragend', handleDragEnd);
	node.addEventListener('dragenter', handleDragEnter);
	node.addEventListener('dragleave', handleDragLeave);
	node.addEventListener('dragover', handleDragOver);
	node.addEventListener('drop', handleDrop);
	node.classList.add('drop-zone');

	node.addEventListener('drop', (e: DragEvent) => {
		console.log(options);
		Object.entries(options.handlers).forEach(([type, handler]) => {
			const data = e.dataTransfer?.getData(type);
			if (data) {
				handler(data);
			}
		});
	});

	return {
		destroy() {
			node.removeEventListener('dragend', handleDragEnd);
			node.removeEventListener('dragenter', handleDragEnter);
			node.removeEventListener('dragleave', handleDragLeave);
			node.removeEventListener('dragover', handleDragOver);
			node.removeEventListener('drop', handleDrop);

			// unregister this node as a drop target for each of the handler types
			Object.keys(options.handlers).forEach((type) => {
				const zones = getZones(type);
				zones.delete(node);
				hightlightOptionsMap.delete(node);
			});
		}
	};
}

const zoneMap = new Map<string, Set<HTMLElement>>();
const optionsMap = new Map<HTMLElement, DzOptions>();

export interface DzOptions {
	type: string;
	hover: string;
	active: string;
}

const defaultOptions: DzOptions = {
	hover: 'drop-zone-hover',
	active: 'drop-zone-active',
	type: 'default'
};

function inactivateZones(zones: Set<HTMLElement>) {
	zones?.forEach((zone) => {
		const opts = optionsMap.get(zone);
		opts && zone.classList.remove(opts.active);
	});
}

function activateZones(zones: Set<HTMLElement>, activeZone: HTMLElement) {
	zones?.forEach((zone) => {
		if (zone !== activeZone && !isChildOf(activeZone, zone)) {
			const opts = optionsMap.get(zone);
			opts && zone.classList.add(opts.active);
		}
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

export function dzTrigger(node: HTMLElement, opts: Partial<DzOptions> | undefined) {
	const options = { ...defaultOptions, ...opts };
	const zones = getZones(options.type);

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
		activateZones(zones, node);
		e.stopPropagation();
	}

	function handleDragEnd(e: DragEvent) {
		node.style.opacity = '1'; // Undo the dimming from `dragstart`
		clone.remove(); // Remove temporary ghost element

		e.stopPropagation();
		inactivateZones(zones);
	}

	node.addEventListener('dragstart', handleDragStart);
	node.addEventListener('dragend', handleDragEnd);

	return {
		destroy() {
			node.removeEventListener('dragstart', handleDragStart);
			node.removeEventListener('dragend', handleDragEnd);
		}
	};
}

export function dzHighlight(node: HTMLElement, opts: Partial<DzOptions> | undefined) {
	const options = { ...defaultOptions, ...opts };
	const zones = getZones(options.type);
	zones.add(node);
	optionsMap.set(node, options);

	function setHover(value: boolean) {
		if (value) {
			// We do this so we can set pointer-events-none on all dropzones from main css file,
			// without it onMouseLeave fires every time a child container is left.
			node.classList.add(defaultOptions.hover);
			node.classList.add(options.hover);
		} else {
			node.classList.remove(defaultOptions.hover);
			node.classList.remove(options.hover);
		}
	}

	function handleDragEnter(e: DragEvent) {
		if (!e.dataTransfer?.types.includes(options.type)) {
			return;
		}
		setHover(true);
		e.stopPropagation();
	}

	function handleDragLeave(e: DragEvent) {
		if (!e.dataTransfer?.types.includes(options.type)) {
			return;
		}
		if (!isChildOf(e.target, node)) {
			setHover(false);
		}
		e.stopPropagation();
	}

	function handleDragEnd(e: DragEvent) {
		setHover(false);
		inactivateZones(zones);
		e.stopPropagation();
	}

	function handleDrop(e: DragEvent) {
		if (!e.dataTransfer?.types.includes(options.type)) {
			return;
		}
		setHover(false);
		inactivateZones(zones);
	}

	function handleDragOver(e: DragEvent) {
		if (!e.dataTransfer?.types.includes(options.type)) {
			e.stopImmediatePropagation(); // Stops event from reaching `on:dragover` on the element
		}
		if (e.dataTransfer?.types.includes(options.type)) e.preventDefault();
	}

	node.addEventListener('dragend', handleDragEnd);
	node.addEventListener('dragenter', handleDragEnter);
	node.addEventListener('dragleave', handleDragLeave);
	node.addEventListener('dragover', handleDragOver);
	node.addEventListener('drop', handleDrop);
	node.classList.add('drop-zone');

	return {
		destroy() {
			node.removeEventListener('dragend', handleDragEnd);
			node.removeEventListener('dragenter', handleDragEnter);
			node.removeEventListener('dragleave', handleDragLeave);
			node.removeEventListener('dragover', handleDragOver);
			node.removeEventListener('drop', handleDrop);
			zones?.delete(node);
		}
	};
}

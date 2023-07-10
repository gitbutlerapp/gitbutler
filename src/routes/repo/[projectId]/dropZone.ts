const zoneMap = new Map<string, Set<HTMLElement>>();

export interface DzOptions {
	type: string;
	hover: string;
	active: string;
}

const defaultOptions: DzOptions = {
	hover: 'drag-zone-hover',
	active: 'drag-zone-active',
	type: 'default'
};

function inactivateZones(zones: Set<HTMLElement>, cssClass: string) {
	zones?.forEach((zone) => {
		zone.classList.remove(cssClass);
	});
}

function activateZones(zones: Set<HTMLElement>, activeZone: HTMLElement, cssClass: string) {
	zones?.forEach((zone) => {
		if (zone !== activeZone && !isChildOf(activeZone, zone)) {
			zone.classList.add(cssClass);
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

	function handleDragStart(e: DragEvent) {
		activateZones(zones, node, options.active);
		e.stopPropagation();
	}

	node.addEventListener('dragstart', handleDragStart);

	return {
		destroy() {
			node.removeEventListener('dragstart', handleDragStart);
		}
	};
}

export function dzHighlight(node: HTMLElement, opts: Partial<DzOptions> | undefined) {
	const options = { ...defaultOptions, ...opts };
	const zones = getZones(options.type);
	zones.add(node);

	function handleDragEnter(e: DragEvent) {
		if (!e.dataTransfer?.types.includes(options.type)) {
			return;
		}
		node.classList.add(options.hover);
		e.stopPropagation();
	}

	function handleDragLeave(e: DragEvent) {
		if (!e.dataTransfer?.types.includes(options.type)) {
			return;
		}
		if (!isChildOf(e.target, node)) {
			node.classList.remove(options.hover);
		}
		e.stopPropagation();
	}

	function handleDragEnd(e: DragEvent) {
		node.classList.remove(options.hover);
		inactivateZones(zones, options.active);
		e.stopPropagation();
	}

	function handleDrop(e: DragEvent) {
		if (!e.dataTransfer?.types.includes(options.type)) {
			return;
		}
		node.classList.remove(options.hover);
		inactivateZones(zones, options.active);
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

export type AddressSpace<T> = {
	items: Array<T>;
	indexByKey: Map<string, number>;
};

export const buildIndexByKey = <T>(
	items: Array<T>,
	getKey: (item: T) => string,
): Map<string, number> => {
	const indexByKey = new Map<string, number>();
	for (const [itemIndex, item] of items.entries()) indexByKey.set(getKey(item), itemIndex);
	return indexByKey;
};

export const getAdjacent = <T>({
	addressSpace,
	selection,
	offset,
	getKey,
	predicate,
}: {
	addressSpace: AddressSpace<T>;
	selection: T;
	offset: -1 | 1;
	getKey: (item: T) => string;
	/** Rejected items are skipped; this predicate does not stop traversal at a group boundary. */
	predicate?: (item: T) => boolean;
}): T | null => {
	const selectionIndex = addressSpace.indexByKey.get(getKey(selection));
	if (selectionIndex === undefined) return null;

	for (
		let index = selectionIndex + offset;
		index >= 0 && index < addressSpace.items.length;
		index += offset
	) {
		const item = addressSpace.items[index];
		if (item !== undefined && (predicate?.(item) ?? true)) return item;
	}
	return null;
};

export const addressSpaceIncludes = <T>(
	addressSpace: AddressSpace<T>,
	item: T,
	getKey: (item: T) => string,
): boolean => addressSpace.indexByKey.has(getKey(item));

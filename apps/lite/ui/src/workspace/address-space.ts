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
}: {
	addressSpace: AddressSpace<T>;
	selection: T;
	offset: -1 | 1;
	getKey: (item: T) => string;
}): T | null => {
	const selectionIndex = addressSpace.indexByKey.get(getKey(selection));
	if (selectionIndex === undefined) return null;

	return addressSpace.items[selectionIndex + offset] ?? null;
};

export const addressSpaceIncludes = <T>(
	addressSpace: AddressSpace<T>,
	item: T,
	getKey: (item: T) => string,
): boolean => addressSpace.indexByKey.has(getKey(item));

export const resolveAddressSpaceSelection = <T>(
	addressSpace: AddressSpace<T>,
	selection: T | null,
	getKey: (item: T) => string,
): T | null =>
	selection !== null && addressSpaceIncludes(addressSpace, selection, getKey)
		? selection
		: (addressSpace.items[0] ?? null);

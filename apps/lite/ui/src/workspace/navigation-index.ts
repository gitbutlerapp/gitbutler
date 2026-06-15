export type NavigationIndex<T> = {
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
	navigationIndex,
	selection,
	offset,
	getKey,
}: {
	navigationIndex: NavigationIndex<T>;
	selection: T;
	offset: -1 | 1;
	getKey: (item: T) => string;
}): T | null => {
	const selectionIndex = navigationIndex.indexByKey.get(getKey(selection));
	if (selectionIndex === undefined) return null;

	return navigationIndex.items[selectionIndex + offset] ?? null;
};

export const navigationIndexIncludes = <T>(
	navigationIndex: NavigationIndex<T>,
	item: T,
	getKey: (item: T) => string,
): boolean => navigationIndex.indexByKey.has(getKey(item));

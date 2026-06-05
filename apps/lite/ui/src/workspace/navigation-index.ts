export type NavigationIndex<T> = {
	items: Array<T>;
	indexByKey: Map<string, number>;
};

const createNavigationIndex = <T>(): NavigationIndex<T> => ({
	items: [],
	indexByKey: new Map<string, number>(),
});

export const buildNavigationIndex = <T>(
	items: Array<T>,
	getKey: (item: T) => string,
): NavigationIndex<T> => {
	const index = createNavigationIndex<T>();

	for (const item of items) {
		const itemIndex = index.items.length;
		index.items.push(item);
		index.indexByKey.set(getKey(item), itemIndex);
	}

	return index;
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

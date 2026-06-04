import { type Operand, operandIdentityKey } from "#ui/operands.ts";

export type NavigationIndex = {
	items: Array<Operand>;
	indexByKey: Map<string, number>;
};

const createNavigationIndex = (): NavigationIndex => ({
	items: [],
	indexByKey: new Map<string, number>(),
});

export const buildNavigationIndex = (items: Array<Operand>): NavigationIndex => {
	const index = createNavigationIndex();

	for (const item of items) {
		const itemIndex = index.items.length;
		index.items.push(item);
		index.indexByKey.set(operandIdentityKey(item), itemIndex);
	}

	return index;
};

export const getAdjacent = ({
	navigationIndex,
	selection,
	offset,
}: {
	navigationIndex: NavigationIndex;
	selection: Operand;
	offset: -1 | 1;
}): Operand | null => {
	const selectionIndex = navigationIndex.indexByKey.get(operandIdentityKey(selection));
	if (selectionIndex === undefined) return null;

	return navigationIndex.items[selectionIndex + offset] ?? null;
};

export const navigationIndexIncludes = (
	navigationIndex: NavigationIndex,
	operand: Operand,
): boolean => navigationIndex.indexByKey.has(operandIdentityKey(operand));

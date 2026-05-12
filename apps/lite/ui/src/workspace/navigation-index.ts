import { type Operand, operandIdentityKey } from "#ui/operands.ts";
import { Array } from "effect";

export type Section = {
	section?: Operand;
	children: Array<Operand>;
};

export type NavigationIndex = {
	items: Array<Operand>;
	sectionStartIndexes: Array<number>;
	sectionIndexByItemIndex: Array<number>;
	indexByKey: Map<string, number>;
};

const createNavigationIndex = (): NavigationIndex => ({
	items: [],
	sectionStartIndexes: [],
	sectionIndexByItemIndex: [],
	indexByKey: new Map<string, number>(),
});

export const buildNavigationIndex = (sections: Array.NonEmptyArray<Section>): NavigationIndex => {
	const index = createNavigationIndex();

	for (const section of sections) {
		const itemsInSection = section.section
			? [section.section, ...section.children]
			: section.children;
		if (itemsInSection.length === 0) continue;

		const sectionIndex = index.sectionStartIndexes.length;
		index.sectionStartIndexes.push(index.items.length);

		for (const item of itemsInSection) {
			const itemIndex = index.items.length;
			index.items.push(item);
			index.sectionIndexByItemIndex.push(sectionIndex);
			index.indexByKey.set(operandIdentityKey(item), itemIndex);
		}
	}

	return index;
};

export const filterNavigationIndex = (
	index: NavigationIndex,
	predicate: (operand: Operand) => boolean,
): NavigationIndex => {
	const filteredIndex = createNavigationIndex();

	const sectionIndexBySourceSectionIndex = new Map<number, number>();

	for (const [itemIndex, item] of index.items.entries()) {
		if (!predicate(item)) continue;

		const sourceSectionIndex = index.sectionIndexByItemIndex[itemIndex];
		if (sourceSectionIndex === undefined) continue;
		let filteredSectionIndex = sectionIndexBySourceSectionIndex.get(sourceSectionIndex);
		if (filteredSectionIndex === undefined) {
			filteredSectionIndex = filteredIndex.sectionStartIndexes.length;
			sectionIndexBySourceSectionIndex.set(sourceSectionIndex, filteredSectionIndex);
			filteredIndex.sectionStartIndexes.push(filteredIndex.items.length);
		}

		const filteredItemIndex = filteredIndex.items.length;
		filteredIndex.items.push(item);
		filteredIndex.sectionIndexByItemIndex.push(filteredSectionIndex);
		filteredIndex.indexByKey.set(operandIdentityKey(item), filteredItemIndex);
	}

	return filteredIndex;
};

export const getAdjacent = ({
	navigationIndex,
	selection,
	offset,
}: {
	navigationIndex: NavigationIndex;
	selection: Operand;
	offset: -1 | 1;
}): Operand | undefined => {
	const selectionIndex = navigationIndex.indexByKey.get(operandIdentityKey(selection));
	if (selectionIndex === undefined) return undefined;

	return navigationIndex.items[selectionIndex + offset];
};

export const getNextSection = ({
	navigationIndex,
	selection,
}: {
	navigationIndex: NavigationIndex;
	selection: Operand;
}): Operand | undefined => {
	const selectionIndex = navigationIndex.indexByKey.get(operandIdentityKey(selection));
	if (selectionIndex === undefined) return undefined;

	const sectionIndex = navigationIndex.sectionIndexByItemIndex[selectionIndex];
	if (sectionIndex === undefined) return undefined;
	const nextSectionStartIndex = navigationIndex.sectionStartIndexes[sectionIndex + 1];
	if (nextSectionStartIndex === undefined) return undefined;

	return navigationIndex.items[nextSectionStartIndex];
};

export const getPreviousSection = ({
	navigationIndex,
	selection,
}: {
	navigationIndex: NavigationIndex;
	selection: Operand;
}): Operand | undefined => {
	const selectionIndex = navigationIndex.indexByKey.get(operandIdentityKey(selection));
	if (selectionIndex === undefined) return undefined;

	const sectionIndex = navigationIndex.sectionIndexByItemIndex[selectionIndex];
	if (sectionIndex === undefined) return undefined;
	const currentSectionStartIndex = navigationIndex.sectionStartIndexes[sectionIndex];
	if (currentSectionStartIndex === undefined) return undefined;

	if (selectionIndex !== currentSectionStartIndex)
		return navigationIndex.items[currentSectionStartIndex];

	const previousSectionStartIndex = navigationIndex.sectionStartIndexes[sectionIndex - 1];
	if (previousSectionStartIndex === undefined) return undefined;

	return navigationIndex.items[previousSectionStartIndex];
};

export const navigationIndexIncludes = (
	navigationIndex: NavigationIndex,
	operand: Operand,
): boolean => navigationIndex.indexByKey.has(operandIdentityKey(operand));

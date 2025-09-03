export function addToArray<T>(array: T[], item: T) {
	if (!array.includes(item)) {
		array.push(item);
	}
}

export function removeFromArray<T>(array: T[], item: T) {
	const index = array.indexOf(item);
	if (index !== -1) {
		array.splice(index, 1);
	}
}

export function sortByDomOrder<T extends Element>(elements: T[]): T[] {
	return elements.sort((a, b) => {
		if (a === b) return 0;
		const pos = a.compareDocumentPosition(b);

		if (pos & Node.DOCUMENT_POSITION_FOLLOWING) {
			return -1; // a comes before b
		}
		if (pos & Node.DOCUMENT_POSITION_PRECEDING) {
			return 1; // a comes after b
		}
		return 0;
	});
}
export async function scrollIntoViewIfNeeded(
	el: HTMLElement,
	behavior: ScrollBehavior = 'smooth',
	root: HTMLElement | null = null // optional scroll container
): Promise<void> {
	return await new Promise((resolve) => {
		// Create observer
		const observer = new IntersectionObserver(
			(entries) => {
				const entry = entries[0];
				if (entry?.isIntersecting && entry.intersectionRatio === 1) {
					observer.disconnect();
					resolve();
				} else {
					el.scrollIntoView({ behavior, block: 'nearest', inline: 'nearest' });
					observer.disconnect();
					resolve();
				}
			},
			{
				root, // if null, defaults to viewport. Otherwise pass scroll container.
				threshold: 1.0 // require 100% visibility
			}
		);

		observer.observe(el);
	});
}

export function isContentEditable(element: HTMLElement): boolean {
	if (!(element instanceof HTMLElement)) {
		return false;
	}
	const contentEditableValue = element.contentEditable.toLowerCase();
	return contentEditableValue === 'true' || contentEditableValue === 'plaintext-only';
}

/**
 * Safely removes an element from an array and returns whether it was found
 */
export function safeRemoveFromArray<T>(array: T[], item: T): boolean {
	const index = array.indexOf(item);
	if (index !== -1) {
		array.splice(index, 1);
		return true;
	}
	return false;
}

/**
 * Adds an element to an array and sorts by DOM order
 */
export function addAndSortByDomOrder<T extends Element>(array: T[], item: T): void {
	addToArray(array, item);
	sortByDomOrder(array);
}

/**
 * Moves an element from one array to another and sorts the destination array
 */
export function moveElementBetweenArrays<T extends Element>(
	sourceArray: T[],
	destinationArray: T[],
	item: T
): boolean {
	if (safeRemoveFromArray(sourceArray, item)) {
		destinationArray.push(item);
		sortByDomOrder(destinationArray);
		return true;
	}
	return false;
}

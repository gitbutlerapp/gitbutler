export function removeFromArray<T>(array: T[], item: T) {
	const index = array.indexOf(item);
	if (index !== -1) {
		array.splice(index, 1);
	}
}

export async function scrollIntoViewIfNeeded(
	el: HTMLElement,
	behavior: ScrollBehavior = 'smooth',
	root: HTMLElement | null = null
): Promise<void> {
	return await new Promise((resolve) => {
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
				root,
				threshold: 1.0
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

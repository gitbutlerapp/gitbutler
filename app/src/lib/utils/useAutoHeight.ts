export function useAutoHeight(element: HTMLTextAreaElement) {
	if (!element) return;

	const elementBorder =
		parseInt(getComputedStyle(element).borderTopWidth) +
		parseInt(getComputedStyle(element).borderBottomWidth);
	element.style.height = 'auto';
	element.style.height = `${element.scrollHeight + elementBorder}px`;
}

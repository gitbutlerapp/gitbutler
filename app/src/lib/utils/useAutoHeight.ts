export function useAutoHeight(element: HTMLTextAreaElement) {
	if (!element) return;
	element.style.height = 'auto';
	element.style.height = `${element.scrollHeight}px`;
}

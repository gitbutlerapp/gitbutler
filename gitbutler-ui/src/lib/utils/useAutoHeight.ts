export function setAutoHeight(element: HTMLTextAreaElement) {
	if (!element) return;

	element.style.height = 'auto';
	element.style.height = `${element.scrollHeight + 2}px`;
}

export function useAutoHeight(event: Event) {
	const textarea = event.target as HTMLTextAreaElement;

	setAutoHeight(textarea);
}

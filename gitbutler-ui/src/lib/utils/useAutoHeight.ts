export function useAutoHeight(event: Event) {
	const textarea = event.target as HTMLTextAreaElement;

	textarea.style.height = 'auto';
	textarea.style.height = `${textarea.scrollHeight + 2}px`;
}

export function useAutoHeight(event: KeyboardEvent) {
	const textarea = event.target as HTMLTextAreaElement;

	textarea.style.height = 'auto';
	textarea.style.height = `${textarea.scrollHeight + 2}px`;
}

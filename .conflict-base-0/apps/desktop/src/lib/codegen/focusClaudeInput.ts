export function focusClaudeInput(stackId: string) {
	// This is a hacky way, but we need the job done until we
	// can figure out a good way of autofocusing text inputs,
	// without too many of them firing at the wrong times.
	const element = document.querySelector(`[data-id="${stackId}"] .ContentEditable__root`);
	if (element instanceof HTMLElement) {
		element?.focus();
	}
}

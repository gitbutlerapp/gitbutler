export function truncate(text: string, maxChars: number, maxLines: number): string {
	if (!text) return text;

	const lines = text.split('\n');

	if (text.length <= maxChars && lines.length <= maxLines) {
		return text;
	}

	const truncatedByLines = lines.slice(0, maxLines).join('\n');

	if (truncatedByLines.length <= maxChars) {
		return truncatedByLines + '…';
	}

	const truncated = text.substring(0, maxChars);

	const lastSpaceIndex = truncated.lastIndexOf(' ');
	const lastTabIndex = truncated.lastIndexOf('\t');
	const lastNewlineIndex = truncated.lastIndexOf('\n');

	const lastSeparatorIndex = Math.max(lastSpaceIndex, lastTabIndex, lastNewlineIndex);

	if (lastSeparatorIndex > 0) {
		return text.substring(0, lastSeparatorIndex) + '…';
	} else {
		// When there is one very long word.
		return text.substring(0, maxChars) + '…';
	}
}

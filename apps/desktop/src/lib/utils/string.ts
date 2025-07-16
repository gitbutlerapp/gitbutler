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

export function rejoinParagraphs(text: string): string {
	const lines = text.split('\n');
	const result: string[] = [];
	let currentParagraph = '';

	for (let i = 0; i < lines.length; i++) {
		const line = lines[i]!;
		const trimmedLine = line.trim();

		// Check if this line should start a new block (headers, lists, code blocks, etc.)
		const isBlockElement = /^(#{1,6}\s|```|~~~|\*\s|-\s|\d+\.\s|>\s|\|\s|$)/.test(trimmedLine);

		// Check if previous line was a block element
		const prevLine = i > 0 ? lines[i - 1]!.trim() : '';
		const prevWasBlock = /^(#{1,6}\s|```|~~~|\*\s|-\s|\d+\.\s|>\s|\|\s)/.test(prevLine);

		// Empty line - signals paragraph break
		if (trimmedLine === '') {
			if (currentParagraph.trim()) {
				result.push(currentParagraph.trim());
				currentParagraph = '';
			}
			continue;
		}

		// Start new paragraph for block elements or after block elements
		if (isBlockElement || prevWasBlock) {
			if (currentParagraph.trim()) {
				result.push(currentParagraph.trim());
				currentParagraph = '';
			}
			result.push(line);
		} else {
			// Continue current paragraph
			if (currentParagraph) {
				currentParagraph += ' ' + trimmedLine;
			} else {
				currentParagraph = trimmedLine;
			}
		}
	}

	// Add any remaining paragraph
	if (currentParagraph.trim()) {
		result.push(currentParagraph.trim());
	}

	return result.join('\n\n');
}

export function truncate(text: string, maxChars: number, maxLines: number): string {
	if (!text) return text;

	const lines = text.split("\n");

	if (text.length <= maxChars && lines.length <= maxLines) {
		return text;
	}

	const truncatedByLines = lines.slice(0, maxLines).join("\n");

	if (truncatedByLines.length <= maxChars) {
		return truncatedByLines + "…";
	}

	const truncated = text.substring(0, maxChars);

	const lastSpaceIndex = truncated.lastIndexOf(" ");
	const lastTabIndex = truncated.lastIndexOf("\t");
	const lastNewlineIndex = truncated.lastIndexOf("\n");

	const lastSeparatorIndex = Math.max(lastSpaceIndex, lastTabIndex, lastNewlineIndex);

	if (lastSeparatorIndex > 0) {
		return text.substring(0, lastSeparatorIndex) + "…";
	} else {
		// When there is one very long word.
		return text.substring(0, maxChars) + "…";
	}
}

export function rejoinParagraphs(text: string): string {
	return text
		.split("\n")
		.reduce((acc, line, index, lines) => {
			const trimmed = line.trim();
			const prevLine = index > 0 ? lines[index - 1]?.trim() || "" : "";

			// Empty line - preserve as paragraph break
			if (!trimmed) {
				return acc + "\n";
			}

			// List items or block elements - preserve spacing
			if (/^(#{1,6}\s|```|~~~|\*\s|-\s|\d+\.\s|>\s|\|\s)/.test(trimmed)) {
				return acc + (acc && !acc.endsWith("\n") ? "\n" : "") + line;
			}

			// First line or after empty line - start new paragraph
			if (!acc || acc.endsWith("\n\n")) {
				return acc + trimmed;
			}

			// Continue paragraph if previous line doesn't end with punctuation
			// and current line doesn't start with capital
			if (!/[.!?]$/.test(prevLine) && !/^[A-Z]/.test(trimmed)) {
				return acc + " " + trimmed;
			}

			// Otherwise, single line break
			return acc + "\n" + trimmed;
		}, "")
		.replace(/\n{3,}/g, "\n\n"); // Normalize multiple newlines to double
}

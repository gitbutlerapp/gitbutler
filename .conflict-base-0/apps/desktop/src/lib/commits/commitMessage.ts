/**
 * Splits a commit message into a title and description.
 *
 * The title is the first line of the message, and the description is everything from the
 * next non-emptyline till the last non-empty line.
 *
 * Only the title will be trimmed (unless otherwise specified), the description will keep its original formatting.
 */
export function splitMessage(message: string, skipTrimming: boolean = false) {
	const lines = message.split("\n");
	if (lines.length === 0) {
		return { title: "", description: "" };
	}

	if (lines.length === 1) {
		return { title: skipTrimming ? message : message.trim(), description: "" };
	}

	const title = skipTrimming ? lines[0]! : lines[0]!.trim();
	let description: string = "";

	// Search for the first and last non-empty lines
	// to determine the description.

	let firstNonEmptyLine = -1;
	let lastNonEmptyLine = -1;
	for (let i = 1; i < lines.length; i++) {
		const line = lines[i]!.trim();
		if (line.length === 0) {
			continue;
		}

		if (firstNonEmptyLine === -1) {
			firstNonEmptyLine = i;
		}
		lastNonEmptyLine = i;
	}

	if (firstNonEmptyLine !== -1 && lastNonEmptyLine !== -1) {
		description = lines.slice(firstNonEmptyLine, lastNonEmptyLine + 1).join("\n");
	}

	return {
		title,
		description,
	};
}

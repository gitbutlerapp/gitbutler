/**
 * Message formatting utilities for GitButler commit messages.
 *
 * This module handles the formatting and parsing of commit messages,
 * allowing for a standardized format in the Git repository while providing
 * UI-friendly versions for editing.
 */

class CommitMessageFormatter {
	/**
	 * Wrap a line of text at 72 characters, preserving list items and quotes.
	 * @param {string} line - The line to wrap
	 * @param {string|null} leading - String to prepend to lines after the first
	 * @param {number|null} indent - Number of spaces to indent
	 * @returns {string} The wrapped text
	 */
	static wrapLine(
		line: string,
		leading: string | null = null,
		indent: number | null = null
	): string {
		const leadingSpaces = line.length - line.trimStart().length;
		// We use `as string[]` to tell TypeScript these are all strings
		const words = line.split(/\s+/).filter(Boolean) as string[];
		let lines = 0;

		let result = '';
		let currentLine = '';

		if (leadingSpaces > 0) {
			result += ' '.repeat(leadingSpaces);
		}

		let currentIndent = '';
		if (indent !== null) {
			currentIndent = ' '.repeat(indent);
		}

		for (let j = 0; j < words.length; j++) {
			const word = words[j];

			if (!word) {
				continue;
			}

			if (currentLine === '') {
				currentLine = word;
			} else if (currentLine.length + word.length + 1 > 72) {
				// Line would be too long, start a new line
				if (lines > 0 && leading !== null) {
					result += leading;
				}
				result += currentLine;
				result += '\n';
				result += currentIndent;
				lines += 1;
				currentLine = word;
			} else {
				// Add word to current line
				currentLine += ' ' + word;
			}

			// If this is the last word and we're not at the end of the input
			if (j === words.length - 1) {
				if (lines > 0 && leading !== null) {
					result += leading;
				}
				result += currentLine.trimEnd();
				currentLine = '';
			}
		}

		return result;
	}

	/**
	 * Turn a multi-line quote into a single line
	 * @param {string} paragraph - The paragraph to unwrap
	 * @returns {string} Unwrapped text
	 */
	static quoteUnwrap(paragraph: string): string {
		let result = '';
		const lines = paragraph.split('\n');

		for (let j = 0; j < lines.length; j++) {
			let line = lines[j] || '';

			// preserve indentation of first line
			if (j === 0) {
				const leadingSpaces = line.length - line.trimStart().length;
				result += ' '.repeat(leadingSpaces);
			}

			line = line.trim();

			if (line.startsWith('>') && j > 0) {
				result += line.replace(/^>\s*/, '').trim();
			} else {
				result += line;
			}
			result += ' ';
		}

		return result.trimEnd();
	}

	/**
	 * Process bullet points in a paragraph
	 * @param {string} paragraph - The paragraph to process
	 * @returns {string} Processed text
	 */
	static bulletUnwrap(paragraph: string): string {
		const possibleBullets = ['*', '-', '+'];

		let result = '';
		const lines = paragraph.split('\n');

		for (let j = 0; j < lines.length; j++) {
			const line = lines[j] || '';

			// if it starts with any of the possible bullets, start a new line
			if (possibleBullets.some((bullet) => line.trim().startsWith(bullet))) {
				if (j > 0) {
					result = result.trimEnd();
					result += '\n';
				}
				result += line;
			} else {
				// it's a continuation of the last bullet
				result += line.trim();
			}

			result += ' ';
		}

		return result.trimEnd();
	}

	/**
	 * Basic paragraph unwrapping
	 * @param {string} paragraph - The paragraph to unwrap
	 * @returns {string} Unwrapped text
	 */
	static simpleUnwrap(paragraph: string): string {
		let result = '';
		const lines = paragraph.split('\n');
		const trailerRegex = /^[!-9;-~]+:\s*.+$/;

		// Process each line in the paragraph
		for (let j = 0; j < lines.length; j++) {
			const line = lines[j] || '';

			// if it's a trailer (RFC 822 grammar), add it to the result
			if (trailerRegex.test(line.trim())) {
				result += line;
				result += '\n';
			} else {
				result += line;
				result += ' ';
			}
		}

		return result.trimEnd();
	}

	/**
	 * Format a user-provided message for storage in a commit.
	 * @param {string} message - The message to format
	 * @returns {string} Formatted message for commit storage
	 */
	static formatForCommit(message: string): string {
		// Split the message into paragraphs
		const paragraphs = message.split('\n\n');

		if (paragraphs.length === 0) {
			return '';
		}

		// Keep the first line as is, this is the subject line
		let result = paragraphs[0] || '';
		result += '\n\n';

		let codeBlock = false;

		// Format the rest of the message with hard wrapping text paragraphs at 72 chars
		if (paragraphs.length > 1) {
			// Process remaining paragraphs
			for (let i = 1; i < paragraphs.length; i++) {
				const paragraph = paragraphs[i] || '';
				const lines = paragraph.split('\n');

				// Process each line in the paragraph
				for (let x = 0; x < lines.length; x++) {
					const line = lines[x] || '';

					if (line.startsWith('```')) {
						codeBlock = !codeBlock;
						result += line;
					} else if (codeBlock || line.length <= 72) {
						result += line;
					} else {
						// is this a list item or quote?
						const isListItem = line.trim().startsWith('* ');
						const isQuote = line.trim().startsWith('> ');

						if (isListItem || isQuote) {
							const leadingSpaces = line.length - line.trimStart().length;

							if (isListItem) {
								result += this.wrapLine(line, null, leadingSpaces + 2);
							} else {
								result += this.wrapLine(line, '> ', leadingSpaces);
							}
						} else {
							result += this.wrapLine(line, null, null);
						}
					}

					if (x < lines.length - 1) {
						result += '\n';
					}
				}

				result += '\n\n';
			}
		}

		return result.trimEnd();
	}

	/**
	 * Parse a commit message back into its user-editable form.
	 * @param {string} formattedMessage - The formatted message to parse
	 * @returns {string} Message in user-editable form
	 */
	static parseForUi(formattedMessage: string): string {
		// Split the message into paragraphs
		const paragraphs = formattedMessage.split('\n\n');

		if (paragraphs.length === 0) {
			return '';
		}

		let codeBlock = false;

		// Keep the first line as is, this is the subject line
		let result = paragraphs[0] || '';
		result += '\n\n';

		if (paragraphs.length > 1) {
			// Process remaining paragraphs
			for (let i = 1; i < paragraphs.length; i++) {
				const paragraph = paragraphs[i] || '';

				// is this a list item or quote?
				const isListItem = paragraph.trim().startsWith('* ');
				const isQuote = paragraph.trim().startsWith('>');
				const startsCodeBlock = paragraph.trim().startsWith('```');
				const endsCodeBlock = paragraph.trim().endsWith('```');

				if (startsCodeBlock) {
					codeBlock = !codeBlock;
				}

				if (codeBlock) {
					result += paragraph;
				} else if (isListItem) {
					result += this.bulletUnwrap(paragraph);
				} else if (isQuote) {
					result += this.quoteUnwrap(paragraph);
				} else {
					result += this.simpleUnwrap(paragraph);
				}

				if (endsCodeBlock) {
					codeBlock = !codeBlock;
				}

				if (i < paragraphs.length - 1) {
					result += '\n\n';
				}
			}
		}

		return result.trimEnd();
	}
}

export default CommitMessageFormatter;

/**
 * Utility for splitting long messages into Discord-friendly chunks
 * that respect logical breaks like newlines
 */

// Maximum Discord message length
export const DISCORD_MAX_LENGTH = 2000;
export const SAFETY_MARGIN = 100; // Leave some room for unexpected characters

/**
 * Split a long text or array of text items into Discord-friendly message chunks
 * @param content Text string or array of text items to split
 * @param separator Separator to use between items when content is an array (default: '\n\n')
 * @param maxLength Maximum length for each message chunk (default: DISCORD_MAX_LENGTH - SAFETY_MARGIN)
 * @returns Array of message chunks ready to be sent
 */
export function splitIntoMessages(
	content: string | string[],
	separator: string = '\n\n',
	maxLength: number = DISCORD_MAX_LENGTH - SAFETY_MARGIN
): string[] {
	// If content is a single string, process it directly
	if (typeof content === 'string') {
		return splitLongString(content, maxLength);
	}

	// If it's an array, handle intelligently combining and splitting items
	const messages: string[] = [];
	let currentMessage = '';

	for (const item of content) {
		// If this item would push us over the limit, start a new message
		if (currentMessage.length + item.length + separator.length > maxLength) {
			if (currentMessage) {
				messages.push(currentMessage);
				currentMessage = '';
			}

			// Handle the case where a single item is too large
			if (item.length > maxLength) {
				messages.push(...splitLongString(item, maxLength));
			} else {
				currentMessage = item;
			}
		} else {
			// Add a separator if this isn't the first item in the message
			if (currentMessage) {
				currentMessage += separator;
			}
			currentMessage += item;
		}
	}

	// Add the last message if there's anything left
	if (currentMessage) {
		messages.push(currentMessage);
	}

	return messages;
}

/**
 * Helper function to split a single long string into chunks
 * @param text Long string to split
 * @param maxLength Maximum length for each chunk
 * @returns Array of text chunks
 */
function splitLongString(text: string, maxLength: number): string[] {
	const chunks: string[] = [];
	let remainingText = text;

	while (remainingText.length > 0) {
		if (remainingText.length <= maxLength) {
			chunks.push(remainingText);
			break;
		}

		// Try to find a newline to split on
		let splitIndex = remainingText.lastIndexOf('\n', maxLength);

		// If no suitable newline, try to find a space
		if (splitIndex <= 0) {
			splitIndex = remainingText.lastIndexOf(' ', maxLength);
		}

		// If no suitable space, split at maximum length
		if (splitIndex <= 0) {
			splitIndex = maxLength;
		}

		chunks.push(remainingText.substring(0, splitIndex));

		// Skip the delimiter (newline or space) when continuing
		const offset =
			remainingText.charAt(splitIndex) === '\n' || remainingText.charAt(splitIndex) === ' ' ? 1 : 0;
		remainingText = remainingText.substring(splitIndex + offset);
	}

	return chunks;
}

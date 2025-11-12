import { getMarkdownString } from '$lib/richText/markdown';

/**
 * Should only be called inside of an editor scope.
 *
 * Gets the current text with the consideration of markdown formatting.
 */
export function getCurrentText(markdown: boolean, maxLength?: number) {
	// If WYSIWYG is enabled, we need to transform the content to markdown strings
	return getMarkdownString(maxLength);
}

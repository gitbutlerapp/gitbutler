import type { RootNode } from 'lexical';

/**
 * We keep this export function in a separate package accessible to both plugins
 * as well as the core editor functionality.
 *
 * TODO: Restructure the richText code so we don't need this separate `utils` package.
 */
export function exportPlaintext(root: RootNode): string {
	// Using `root.getTextContent()` adds extra blank lines between paragraphs, since
	// normally paragraphs have a bottom margin (that we removed).
	const children = root.getChildren();
	const paragraphTexts = children.map((child) => child.getTextContent());
	const text = paragraphTexts.join('\n');
	return text;
}

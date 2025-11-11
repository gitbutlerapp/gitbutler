import { getMarkdownString } from '$lib/richText/markdown';
import { isInlineCodeNode } from '$lib/richText/node/inlineCode';
import {
	$getRoot as getRoot,
	$isTextNode as isTextNode,
	$isElementNode as isElementNode,
	type LexicalNode
} from 'lexical';

/**
 * Gets text content with inline code nodes wrapped in backticks
 */
function getTextContentWithInlineCode(): string {
	const root = getRoot();
	const parts: string[] = [];

	root.getChildren().forEach((child) => {
		if (!isElementNode(child)) return;

		child.getChildren().forEach((node: LexicalNode) => {
			if (isInlineCodeNode(node)) {
				parts.push(`\`${node.getTextContent()}\``);
			} else if (isTextNode(node)) {
				parts.push(node.getTextContent());
			} else {
				parts.push(node.getTextContent());
			}
		});
		parts.push('\n');
	});

	// Remove trailing newline
	return parts.join('').replace(/\n$/, '');
}

/**
 * Should only be called inside of an editor scope.
 *
 * Gets the current text with the consideration of markdown formatting.
 */
export function getCurrentText(markdown: boolean, maxLength?: number) {
	// If WYSIWYG is enabled, we need to transform the content to markdown strings
	if (markdown) return getMarkdownString(maxLength);
	return getTextContentWithInlineCode();
}

import { createInlineCodeNode } from '$lib/richText/node/inlineCode';
import { $isParagraphNode, ParagraphNode } from 'lexical';
import type { ElementTransformer, Transformer } from '@lexical/markdown';

/**
 * A transformer used for exporting to markdown, and having paragraphs
 * separated by `\n\n` instead of just a single newline.
 */
export const ParagraphMarkdownTransformer: ElementTransformer = {
	dependencies: [ParagraphNode],
	export: (node, traverseChildren) => {
		if ($isParagraphNode(node)) {
			const nextSibling = node.getNextSibling();
			const text = traverseChildren(node);
			if (nextSibling !== null) {
				return `${text}\n`;
			}
			return text;
		}
		return null;
	},
	regExp: /./,
	replace: () => false,
	type: 'element'
};

export const INLINE_CODE_TRANSFORMER: Transformer = {
	type: 'text-match',
	importRegExp: /(`[^`]+`)/,
	regExp: /(`[^`]+`)/,
	replace: (textNode, match) => {
		const codeNode = createInlineCodeNode(match[1]);
		textNode.replace(codeNode);
	},
	trigger: '`',
	dependencies: []
};

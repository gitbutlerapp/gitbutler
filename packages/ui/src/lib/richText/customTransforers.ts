import { createInlineCodeNode } from '$lib/richText/node/inlineCode';
import { $isParagraphNode, ParagraphNode } from 'lexical';
import type { ElementTransformer, Transformer } from '@lexical/markdown';

/**
 * A transformer used for exporting to markdown, where each paragraph
 * becomes its own line separated by `\n`.
 *
 * NOTE: This transformer is export-only and should never match during typing.
 * The regExp is set to never match to prevent interference with user input.
 */
export const PARAGRAPH_TRANSFORMER: ElementTransformer = {
	dependencies: [ParagraphNode],
	export: (node, traverseChildren) => {
		if ($isParagraphNode(node)) {
			const nextSibling = node.getNextSibling();
			const text = traverseChildren(node);
			// Each paragraph becomes a line, separated by a single newline
			if (nextSibling !== null) {
				return `${text}`;
			}
			return text;
		}
		return null;
	},
	regExp: /(?:)/,
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

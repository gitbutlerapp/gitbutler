import { $isParagraphNode, ParagraphNode } from 'lexical';
import type { ElementTransformer } from '@lexical/markdown';

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

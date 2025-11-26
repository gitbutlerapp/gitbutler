import { createInlineCodeNode } from '$lib/richText/node/inlineCode';
import type { Transformer } from '@lexical/markdown';

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

import { EmojiNode } from '../node/emoji';
import {
	HeadingNode,
	QuoteNode,
	ListNode,
	ListItemNode,
	HorizontalRuleNode,
	ImageNode,
	KeywordNode,
	HashtagNode,
	AutoLinkNode,
	LinkNode,
	CodeNode,
	CodeHighlightNode,
	TableNode,
	TableCellNode,
	TableRowNode,
	type EditorThemeClasses
} from 'svelte-lexical';

export function standardConfig(args: {
	namespace: string;
	theme: EditorThemeClasses;
	onError: (error: unknown) => void;
}) {
	const { namespace, theme, onError } = args;
	return {
		theme,
		namespace,
		onError,
		nodes: [
			LinkNode,
			AutoLinkNode,
			ListNode,
			ListItemNode,
			TableNode,
			TableCellNode,
			TableRowNode,
			HorizontalRuleNode,
			ImageNode,
			CodeNode,
			HeadingNode,
			LinkNode,
			ListNode,
			ListItemNode,
			QuoteNode,
			HashtagNode,
			CodeHighlightNode,
			EmojiNode,
			KeywordNode
		]
	};
}

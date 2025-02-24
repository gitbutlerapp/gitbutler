import { EmojiNode } from '$lib/richText/node/emoji';
import { GhostText } from '$lib/richText/node/ghostText';
import { InlineCodeNode } from '$lib/richText/node/inlineCode';
import { MentionNode } from '$lib/richText/node/mention';
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
	type EditorThemeClasses,
	$createParagraphNode,
	$createTextNode,
	$getRoot
} from 'svelte-lexical';
import type {
	EditorState,
	HTMLConfig,
	Klass,
	LexicalEditor,
	LexicalNode,
	LexicalNodeReplacement
} from 'lexical';

export type InitialEditorStateType =
	| null
	| string
	| EditorState
	| ((editor: LexicalEditor) => void);

export type InitialConfigType = Readonly<{
	editor__DEPRECATED?: LexicalEditor | null;
	namespace: string;
	nodes?: ReadonlyArray<Klass<LexicalNode> | LexicalNodeReplacement>;
	onError: (error: Error, editor: LexicalEditor) => void;
	editable?: boolean;
	theme?: EditorThemeClasses;
	editorState?: InitialEditorStateType;
	html?: HTMLConfig;
}>;

export function standardConfig(args: {
	initialText?: string;
	namespace: string;
	theme: EditorThemeClasses;
	onError: (error: unknown) => void;
}): InitialConfigType {
	const { namespace, theme, onError, initialText } = args;
	return {
		editable: true,
		theme,
		namespace,
		onError,
		editorState: (editor) => {
			if (initialText) {
				editor.update(() => {
					const paragraph = $createParagraphNode();
					const text = $createTextNode(initialText);
					paragraph.append(text);
					$getRoot().append(paragraph);
					$getRoot().selectEnd();
				});
			}
		},
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
			KeywordNode,
			InlineCodeNode,
			MentionNode,
			GhostText
		]
	};
}

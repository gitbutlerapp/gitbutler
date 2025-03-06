import {
	$convertFromMarkdownString as convertFromMarkdownString,
	$convertToMarkdownString as convertToMarkdownString
} from '@lexical/markdown';
import {
	$getRoot as getRoot,
	type LexicalEditor,
	$createParagraphNode as createParagraphNode,
	$createTextNode as createTextNode
} from 'lexical';
import { ALL_TRANSFORMERS } from 'svelte-lexical';

export function updateEditorToMarkdown(editor: LexicalEditor | undefined) {
	editor?.update(() => {
		const text = getRoot().getTextContent();
		convertFromMarkdownString(text, ALL_TRANSFORMERS);
	});
}

export function updateEditorToPlaintext(editor: LexicalEditor | undefined) {
	editor?.update(() => {
		const text = convertToMarkdownString(ALL_TRANSFORMERS);
		const root = getRoot();
		root.clear();
		const paragraph = createParagraphNode();
		paragraph.append(createTextNode(text));
		root.append(paragraph);
	});
}

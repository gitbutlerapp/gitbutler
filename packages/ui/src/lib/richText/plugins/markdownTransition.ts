import { updateEditorToMarkdown, updateEditorToPlaintext } from '$lib/richText/markdown';
import { type LexicalEditor } from 'lexical';

// It looks like
export default class MarkdownTransitionPlugin {
	private editor: LexicalEditor | undefined;
	private maxLength?: number;

	constructor(maxLength?: number) {
		this.maxLength = maxLength;
	}

	setEditor(editor: LexicalEditor) {
		this.editor = editor;
	}

	setMaxLength(value: number) {
		this.maxLength = value;
	}

	setMarkdown(markdown: boolean) {
		if (this.editor) {
			if (markdown) {
				updateEditorToMarkdown(this.editor);
			} else {
				updateEditorToPlaintext(this.editor, this.maxLength);
			}
		}
	}
}

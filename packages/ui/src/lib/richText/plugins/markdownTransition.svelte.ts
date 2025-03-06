import { updateEditorToMarkdown, updateEditorToPlaintext } from '../markdown';
import type { LexicalEditor } from 'lexical';

export default class MarkdownTransitionPlugin {
	private editor = $state<LexicalEditor>();
	private markdown = $state<boolean>();

	constructor(markdown: boolean) {
		this.markdown = markdown;
		$effect(() => {
			if (this.editor) {
				if (this.markdown) {
					updateEditorToMarkdown(this.editor);
				} else {
					updateEditorToPlaintext(this.editor);
				}
			}
		});
	}

	setEditor(editor: LexicalEditor) {
		this.editor = editor;
	}

	setMarkdown(markdown: boolean) {
		this.markdown = markdown;
	}
}

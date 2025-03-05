import type RichTextEditor from '@gitbutler/ui/RichTextEditor.svelte';
import type OldRichTextEditor from '@gitbutler/ui/old_RichTextEditor.svelte';

export default class RichText {
	private _oldRichTextEditor = $state<ReturnType<typeof OldRichTextEditor>>();
	private _richTextEditor = $state<ReturnType<typeof RichTextEditor>>();

	reset() {
		const editor = this._oldRichTextEditor?.getEditor();
		editor?.commands.clearContent();
	}

	clearEditor() {
		const editor = this._richTextEditor;
		editor?.clear();
	}

	get richTextEditor() {
		return this._richTextEditor;
	}

	set richTextEditor(value: ReturnType<typeof RichTextEditor> | undefined) {
		this._richTextEditor = value;
	}
}

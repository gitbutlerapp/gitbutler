import type RichTextEditor from '@gitbutler/ui/RichTextEditor.svelte';

export default class RichText {
	private _richTextEditor = $state<ReturnType<typeof RichTextEditor>>();

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

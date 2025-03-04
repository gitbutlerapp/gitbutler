import type MentionSuggestions from '$lib/components/chat/MentionSuggestions.svelte';
import type RichTextEditor from '@gitbutler/ui/RichTextEditor.svelte';
import type { MentionNodeAttrs, SuggestionProps } from '@gitbutler/ui/old_RichTextEditor.svelte';
import type OldRichTextEditor from '@gitbutler/ui/old_RichTextEditor.svelte';

export default class RichText {
	private _oldRichTextEditor = $state<ReturnType<typeof OldRichTextEditor>>();
	private _richTextEditor = $state<ReturnType<typeof RichTextEditor>>();
	private _mentionSuggestions = $state<ReturnType<typeof MentionSuggestions>>();
	private _suggestions = $state<MentionNodeAttrs[]>();
	private _selectSuggestion = $state<(id: MentionNodeAttrs) => void>();

	reset() {
		const editor = this._oldRichTextEditor?.getEditor();
		editor?.commands.clearContent();
		this._suggestions = undefined;
		this._selectSuggestion = undefined;
	}

	clearEditor() {
		const editor = this._richTextEditor;
		editor?.clear();
	}

	onSuggestionStart(props: SuggestionProps) {
		this._suggestions = props.items;
		this._selectSuggestion = (item: MentionNodeAttrs) => {
			props.command(item);
		};
	}

	onSuggestionUpdate(props: SuggestionProps) {
		this._suggestions = props.items;
		this._selectSuggestion = (item: MentionNodeAttrs) => {
			props.command(item);
		};
	}

	onSuggestionExit() {
		this._suggestions = undefined;
		this._selectSuggestion = undefined;
	}

	onSuggestionKeyDown(event: KeyboardEvent): boolean {
		if (event.key === 'Escape') {
			this._suggestions = undefined;
			this._selectSuggestion = undefined;
			return true;
		}

		if (event.key === 'Enter') {
			if (this._mentionSuggestions) {
				this._mentionSuggestions.onEnter();
			}
			event.preventDefault();
			event.stopPropagation();
			return true;
		}

		if (event.key === 'ArrowUp') {
			if (this._mentionSuggestions) {
				this._mentionSuggestions.onArrowUp();
			}
			return true;
		}

		if (event.key === 'ArrowDown') {
			if (this._mentionSuggestions) {
				this._mentionSuggestions.onArrowDown();
			}
			return true;
		}

		return false;
	}

	get richTextEditor() {
		return this._richTextEditor;
	}

	set richTextEditor(value: ReturnType<typeof RichTextEditor> | undefined) {
		this._richTextEditor = value;
	}

	get mentionSuggestions() {
		return this._mentionSuggestions;
	}

	set mentionSuggestions(value: ReturnType<typeof MentionSuggestions> | undefined) {
		this._mentionSuggestions = value;
	}

	get suggestions() {
		return this._suggestions;
	}

	set suggestions(value: MentionNodeAttrs[] | undefined) {
		this._suggestions = value;
	}

	get selectSuggestion() {
		return this._selectSuggestion;
	}

	set selectSuggestion(value: ((id: MentionNodeAttrs) => void) | undefined) {
		this._selectSuggestion = value;
	}
}

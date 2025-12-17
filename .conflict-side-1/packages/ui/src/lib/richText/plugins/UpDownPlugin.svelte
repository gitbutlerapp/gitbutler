<script lang="ts">
	import { setEditorText } from '$lib/richText/selection';
	import { mergeUnlisten } from '$lib/utils/mergeUnlisten';
	import {
		$getRoot as getRoot,
		$getSelection as getSelection,
		COMMAND_PRIORITY_NORMAL,
		$isRangeSelection as isRangeSelection,
		KEY_DOWN_COMMAND
	} from 'lexical';
	import { getEditor } from 'svelte-lexical';

	type Props = { historyLookup: (offset: number) => Promise<string | undefined> };

	const { historyLookup }: Props = $props();

	const editor = getEditor();
	let offset = $state(0);
	let active = $state(false);

	async function applyFromHistory(offset: number) {
		const entry = await historyLookup(offset);
		if (entry) {
			setEditorText(editor, entry);
			return true;
		}
		return false;
	}

	function canActivate() {
		const selection = getSelection();
		if (!isRangeSelection(selection)) return false;
		if (!selection.isCollapsed()) return false;
		if (selection.anchor.getNode().getPreviousSibling()) return false;
		if (selection.anchor.offset > 0) return false;

		const contentSize = getRoot().getTextContentSize();
		if (contentSize !== 0) return false;
		return true;
	}

	function handleKeyboardEvent(e: KeyboardEvent): boolean {
		const isKeyMatch = e.key === 'ArrowUp' || e.key === 'ArrowDown';
		const validEnv = active || canActivate();
		if (!isKeyMatch || !validEnv) {
			active = false;
			offset = 0;
			return false;
		}

		active = true;
		e.preventDefault();

		switch (e.key) {
			case 'ArrowUp':
				applyFromHistory(offset++);
				break;
			case 'ArrowDown':
				if (offset === 0) {
					setEditorText(editor, '');
					return false;
				}
				applyFromHistory(--offset);
		}
		return true;
	}

	$effect(() => {
		return mergeUnlisten(
			editor.registerCommand(KEY_DOWN_COMMAND, handleKeyboardEvent, COMMAND_PRIORITY_NORMAL)
		);
	});
</script>

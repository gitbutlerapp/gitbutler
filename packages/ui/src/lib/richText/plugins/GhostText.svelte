<script lang="ts">
	import { getEditor } from '../context';
	import { createGhostTextNode } from '../node/ghostText';
	import { insertNodeAtCaret, insertTextAtCaret } from '../selection';
	import { COMMAND_PRIORITY_CRITICAL, KEY_ESCAPE_COMMAND, KEY_TAB_COMMAND } from 'lexical';

	type Props = {
		onSelection: (text: string) => void;
	};

	const { onSelection }: Props = $props();

	let textContent = $state<string>();

	const editor = getEditor();

	function handleEscape(event: KeyboardEvent): boolean {
		if (!textContent) {
			return false;
		}

		textContent = undefined;

		event.preventDefault();
		event.stopPropagation();
		return true;
	}

	function handleTab(event: KeyboardEvent): boolean {
		if (!textContent) {
			return false;
		}

		onSelection(textContent);
    textContent = undefined;

		event.preventDefault();
		event.stopPropagation();
		return true;
	}

	// Register listeners
	$effect(() => {
		const unregisterTab = editor.registerCommand(
			KEY_TAB_COMMAND,
			handleTab,
			COMMAND_PRIORITY_CRITICAL
		);

		const unregisterEscape = editor.registerCommand(
			KEY_ESCAPE_COMMAND,
			handleEscape,
			COMMAND_PRIORITY_CRITICAL
		);

		return () => {
			unregisterTab();
			unregisterEscape();
		};
	});

	// Insert the ghost text
	export function setText(text: string) {
		textContent = text;
		// const ghostText = createGhostT-extNode(text);
		insertTextAtCaret(editor, 'bla');
	}
</script>

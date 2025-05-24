<script lang="ts">
	import { getEditor } from '$lib/richText/context';
	import {
		insertGhostTextAtCaret,
		removeAllGhostText,
		replaceGhostTextWithText
	} from '$lib/richText/node/ghostText';
	import { CLICK_COMMAND, COMMAND_PRIORITY_CRITICAL, KEY_DOWN_COMMAND } from 'lexical';

	type Props = {
		onSelection?: (text: string) => void;
	};

	const { onSelection }: Props = $props();

	let textContent = $state<string>();

	const editor = getEditor();

	export function reset() {
		textContent = undefined;
		removeAllGhostText(editor);
	}

	function handleEscape(event: KeyboardEvent): boolean {
		if (!textContent) {
			return false;
		}

		reset();

		event.preventDefault();
		event.stopPropagation();
		return true;
	}

	function handleTab(event: KeyboardEvent): boolean {
		if (!textContent) {
			return false;
		}

		replaceGhostTextWithText(editor);
		onSelection?.(textContent);
		reset();

		event.preventDefault();
		event.stopPropagation();
		return true;
	}

	function handleKeyDown(event: KeyboardEvent): boolean {
		if (event.key === 'Escape') {
			return handleEscape(event);
		} else if (event.key === 'Tab') {
			return handleTab(event);
		}

		if (
			[
				'Shift',
				'Control',
				'Meta',
				'Alt',
				'ArrowUp',
				'ArrowDown',
				'ArrowLeft',
				'ArrowRight'
			].includes(event.key)
		) {
			return false;
		}

		reset();
		return false;
	}

	function handleClick(): boolean {
		if (!textContent) {
			return false;
		}
		reset();
		return true;
	}

	// Register listeners
	$effect(() => {
		const unregister = editor.registerCommand(
			KEY_DOWN_COMMAND,
			handleKeyDown,
			COMMAND_PRIORITY_CRITICAL
		);

		const unregisterClick = editor.registerCommand(
			CLICK_COMMAND,
			handleClick,
			COMMAND_PRIORITY_CRITICAL
		);

		return () => {
			unregister();
			unregisterClick();
		};
	});

	// Insert the ghost text
	export function setText(text: string) {
		if (textContent) {
			return;
		}
		textContent = text;
		insertGhostTextAtCaret(editor, text);
	}
</script>

<!--
@component
This component overrides enter key command, in order to customize the behavior
of the Enter key in plain-text mode to handle indentation and bullets.
-->
<script lang="ts" module>
	export function handleEnter() {
		const selection = getSelection();
		if (!isRangeSelection(selection)) return false;

		const anchor = selection.anchor;
		const node = anchor.getNode();

		// Only handle if we're in a text node
		if (!isTextNode(node)) return false;

		// Find the current line by looking backwards from cursor position
		const offset = anchor.offset;
		const textContent = node.getTextContent();

		// Get text before cursor in current node
		const textBeforeCursor = textContent.substring(0, offset);

		// Find the start of the current line (last \n before cursor, or start of text)
		const lastLineBreakIndex = textBeforeCursor.lastIndexOf('\n');
		const currentLineStart = lastLineBreakIndex === -1 ? 0 : lastLineBreakIndex + 1;
		const currentLineText = textBeforeCursor.substring(currentLineStart);

		// Parse indentation and bullets from current line
		const indent = parseIndent(currentLineText);
		const bullet = parseBullet(currentLineText);

		let newIndent = bullet ? bullet.prefix : indent;

		if (bullet?.number) {
			// Parse and increment numeric bullet point
			const padding = bullet.prefix.length - bullet.prefix.trimStart().length;
			newIndent = bullet.prefix.substring(0, padding) + (bullet.number + 1) + '. ';
		}

		// Check if we're on an empty bullet line
		const trimmedLine = currentLineText.trim();
		if (bullet && trimmedLine === bullet.prefix.trim()) {
			// Empty bullet line - remove the bullet and just insert a line break
			const startOfBullet = offset - (currentLineText.length - currentLineStart);
			const beforeBullet = textContent.substring(0, startOfBullet);
			const afterCursor = textContent.substring(offset);
			node.setTextContent(beforeBullet + afterCursor);

			// Move cursor to after the removed bullet
			node.select(beforeBullet.length, beforeBullet.length);
			return true;
		}

		// Insert line break and indentation
		const textAfterCursor = textContent.substring(offset);
		const textBeforeCursorFinal = textContent.substring(0, offset);

		// Split the text node at cursor
		node.setTextContent(textBeforeCursorFinal);

		// Insert line break
		const lineBreak = createLineBreakNode();
		node.insertAfter(lineBreak);

		// Insert indented text after line break
		const newTextNode = createTextNode(newIndent + textAfterCursor);
		lineBreak.insertAfter(newTextNode);

		// Move cursor to after the indent
		newTextNode.select(newIndent.length, newIndent.length);

		return true;
	}
</script>

<script lang="ts">
	import { parseIndent, parseBullet } from '$lib/richText/linewrap';

	import {
		$createTextNode as createTextNode,
		$createLineBreakNode as createLineBreakNode,
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection,
		$isTextNode as isTextNode,
		COMMAND_PRIORITY_CRITICAL,
		INSERT_LINE_BREAK_COMMAND
	} from 'lexical';
	import { getEditor } from 'svelte-lexical';

	const editor = getEditor();

	$effect(() => {
		return editor.registerCommand(
			INSERT_LINE_BREAK_COMMAND,
			handleEnter,
			COMMAND_PRIORITY_CRITICAL
		);
	});
</script>

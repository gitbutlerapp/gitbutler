<!--
@component
This component overrides enter key command to handle indentation and bullets in rich text mode.
-->
<script lang="ts" module>
	export function handleEnter(event: KeyboardEvent | null) {
		// Prevent the default browser behavior that creates extra paragraphs
		if (event) {
			event.preventDefault();
		}

		const selection = getSelection();
		if (!isRangeSelection(selection)) return false;

		const anchor = selection.anchor;
		const node = anchor.getNode();

		// Only handle if we're in a text node
		if (!isTextNode(node)) return false;

		// Get the parent paragraph
		const parent = node.getParent();
		if (!isParagraphNode(parent)) return false;

		// Get the current paragraph text
		const currentLineText = parent.getTextContent();
		const offset = anchor.offset;

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
			// Clear the bullet from the current paragraph
			const children = parent.getChildren();
			for (const child of children) {
				child.remove();
			}
			// Create new paragraph and move cursor there
			const newParagraph = createParagraphNode();
			parent.insertAfter(newParagraph);
			newParagraph.select();
			return true;
		}

		// Split the paragraph at the cursor position
		const textContent = node.getTextContent();
		const textAfterCursor = textContent.substring(offset);
		const textBeforeCursor = textContent.substring(0, offset);

		// Update current node's text to everything before cursor
		node.setTextContent(textBeforeCursor);

		// Create new paragraph with indented text
		const newParagraph = createParagraphNode();
		const newTextNode = createTextNode(newIndent + textAfterCursor);
		newParagraph.append(newTextNode);

		// Insert the new paragraph after the current one
		parent.insertAfter(newParagraph);

		// Set selection to after the indent in the new paragraph
		// Use explicit selection API to ensure it's set correctly
		const newSelection = createRangeSelection();
		newSelection.anchor.set(newTextNode.getKey(), newIndent.length, 'text');
		newSelection.focus.set(newTextNode.getKey(), newIndent.length, 'text');
		setSelection(newSelection);

		return true;
	}
</script>

<script lang="ts">
	import { parseIndent, parseBullet } from '$lib/richText/linewrap';

	import {
		$createTextNode as createTextNode,
		$createParagraphNode as createParagraphNode,
		$createRangeSelection as createRangeSelection,
		$getSelection as getSelection,
		$setSelection as setSelection,
		$isRangeSelection as isRangeSelection,
		$isTextNode as isTextNode,
		$isParagraphNode as isParagraphNode,
		COMMAND_PRIORITY_CRITICAL,
		KEY_ENTER_COMMAND
	} from 'lexical';
	import { getEditor } from 'svelte-lexical';

	const editor = getEditor();

	$effect(() => {
		const unregisterEnter = editor.registerCommand(
			KEY_ENTER_COMMAND,
			handleEnter,
			COMMAND_PRIORITY_CRITICAL
		);

		return () => {
			unregisterEnter();
		};
	});
</script>

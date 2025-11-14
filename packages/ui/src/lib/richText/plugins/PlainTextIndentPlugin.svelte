<!--
@component
This component overrides enter key command, in order to customize the behavior
of the Enter key in plain-text mode to create new paragraphs.
-->
<script lang="ts">
	import { parseIndent, parseBullet } from '$lib/richText/linewrap';

	import {
		$createTextNode as createTextNode,
		$createParagraphNode as createParagraphNode,
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection,
		$isParagraphNode as isParagraphNode,
		$isTextNode as isTextNode,
		COMMAND_PRIORITY_HIGH,
		INSERT_PARAGRAPH_COMMAND
	} from 'lexical';
	import { getEditor } from 'svelte-lexical';

	const editor = getEditor();

	function handleEnter() {
		const selection = getSelection();
		if (!isRangeSelection(selection)) return false;

		const anchor = selection.anchor;
		const node = anchor.getNode();
		const paragraph = node.getParent();

		if (!isParagraphNode(paragraph)) return false;

		const text = paragraph.getTextContent();
		const indent = parseIndent(text);
		const bullet = parseBullet(text);

		let newIndent = bullet ? bullet.prefix : indent;

		if (bullet?.number) {
			// Parse and increment numeric bullet point.
			const padding = bullet.prefix.length - bullet.prefix.trimStart().length;
			newIndent = bullet.prefix.substring(0, padding) + (bullet.number + 1) + '. ';
		}

		if (bullet && text.length === bullet.prefix.length) {
			// Empty bullet line - remove it
			paragraph.remove();
		} else {
			// Split the current paragraph at cursor position
			const offset = anchor.offset;

			// Only handle if we're in a text node
			if (!isTextNode(node)) return false;

			const textContent = node.getTextContent();
			const beforeText = textContent.substring(0, offset);
			const afterText = textContent.substring(offset);

			// Update current node with text before cursor
			node.setTextContent(beforeText);

			// Create new paragraph with indentation and remaining text
			const newParagraph = createParagraphNode();
			const newTextNode = createTextNode(newIndent + afterText);
			newParagraph.append(newTextNode);
			paragraph.insertAfter(newParagraph);

			// Move cursor to start of new paragraph (after indent)
			newTextNode.select(newIndent.length, newIndent.length);
		}

		return true;
	}

	$effect(() => {
		editor.registerCommand(INSERT_PARAGRAPH_COMMAND, handleEnter, COMMAND_PRIORITY_HIGH);
	});
</script>

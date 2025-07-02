<!--
@component
This component overrides enter key command, in order to customize the behavior
of the Enter key.
-->
<script lang="ts">
	import { parseIndent, parseBullet } from '$lib/richText/linewrap';

	import {
		TextNode,
		LineBreakNode,
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection,
		$insertNodes as insertNodes,
		COMMAND_PRIORITY_HIGH,
		INSERT_PARAGRAPH_COMMAND
	} from 'lexical';
	import { getEditor } from 'svelte-lexical';

	const editor = getEditor();

	function handleEnter() {
		const selection = getSelection();
		if (!isRangeSelection(selection)) return false;

		const anchor = selection.anchor;
		const node = anchor.getNode(); // Current line.

		const text = node.getTextContent();
		const indent = parseIndent(text);
		const bullet = parseBullet(text);

		let newIndent = bullet ? bullet.prefix : indent;

		if (bullet?.number) {
			// Parse and increment numeric bullet point.
			const padding = bullet.prefix.length - bullet.prefix.trimStart().length;
			newIndent = bullet.prefix.substring(0, padding) + (bullet.number + 1) + '. ';
		}

		if (bullet && text.length === bullet.prefix.length) {
			node.remove();
		} else {
			insertNodes([new LineBreakNode(), new TextNode(newIndent)]);
		}

		return true;
	}

	$effect(() => {
		editor.registerCommand(INSERT_PARAGRAPH_COMMAND, handleEnter, COMMAND_PRIORITY_HIGH);
	});
</script>

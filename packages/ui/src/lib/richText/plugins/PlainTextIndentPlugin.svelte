<!--
@component
This component overrides enter key command, in order to customize the behavior
of the Enter key.
-->
<script lang="ts">
	import { parseIndent, parseBullet } from '$lib/richText/plugins/linewrap';
	import { mergeUnlisten } from '$lib/utils/mergeUnlisten';

	import {
		TextNode,
		LineBreakNode,
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection,
		$insertNodes as insertNodes,
		COMMAND_PRIORITY_CRITICAL,
		KEY_ENTER_COMMAND
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
			const leadingSpaces = bullet.prefix.length - bullet.prefix.trimStart().length;
			newIndent = bullet.prefix.substring(0, leadingSpaces) + (bullet.number + 1) + '. ';
		}

		insertNodes([new LineBreakNode(), new TextNode(newIndent)]);
		return true; // Prevent default Enter handling.
	}

	$effect(() => {
		return mergeUnlisten(
			/**
			 * The default handler for this command dispatches the
			 * `INSERT_LINE_BREAK` command, but we override upstream
			 * since there were some behavioral differences between
			 * storybook and in-app experiences.
			 */
			editor.registerCommand(
				KEY_ENTER_COMMAND,
				(e: KeyboardEvent) => {
					e.preventDefault();
					handleEnter();
					return true;
				},
				COMMAND_PRIORITY_CRITICAL
			)
		);
	});
</script>

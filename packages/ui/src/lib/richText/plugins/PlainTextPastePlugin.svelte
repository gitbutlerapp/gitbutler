<!--
@component
This plugin strips all formatting from pasted content, only allowing plain text to be inserted.
Use this when you want a simplified rich text mode without formatted paste support.
-->
<script lang="ts">
	import {
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection,
		COMMAND_PRIORITY_HIGH,
		PASTE_COMMAND
	} from 'lexical';
	import { getEditor } from 'svelte-lexical';

	const editor = getEditor();

	$effect(() => {
		const unregisterPaste = editor.registerCommand(
			PASTE_COMMAND,
			(event: ClipboardEvent) => {
				// Get plain text from clipboard
				const text = event?.clipboardData?.getData('text/plain');

				if (text) {
					event.preventDefault();

					// Insert as plain text only, stripping all formatting
					editor.update(() => {
						const selection = getSelection();
						if (isRangeSelection(selection)) {
							selection.insertText(text);
						}
					});

					return true; // Prevent default paste behavior
				}

				return false;
			},
			COMMAND_PRIORITY_HIGH
		);

		return () => {
			unregisterPaste();
		};
	});
</script>

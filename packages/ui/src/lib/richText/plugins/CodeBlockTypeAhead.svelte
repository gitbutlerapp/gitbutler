<!--
@component
This plugin detects triple backticks anywhere on a line and automatically
converts them into a code block.

The code block is inserted on its own line, splitting the current line at the
triple backtick position. An empty line is added after the code block to allow
the user to continue typing.

This is needed because the default markdown shortcuts only match at the beginning
of a paragraph, but in plaintext mode we work with a single paragraph.
-->
<script lang="ts">
	import TypeAhead, { type Match } from '$lib/richText/plugins/TypeAhead.svelte';
	import { mergeUnlisten } from '$lib/utils/mergeUnlisten';
	import {
		$createLineBreakNode as createLineBreakNode,
		$createTextNode as createTextNode,
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection,
		$isTextNode as isTextNode,
		COMMAND_PRIORITY_HIGH,
		KEY_BACKSPACE_COMMAND,
		KEY_DELETE_COMMAND
	} from 'lexical';
	import { CodeNode, $createCodeNode as createCodeNode } from 'svelte-lexical';
	import { getEditor } from 'svelte-lexical';

	// Matches ``` anywhere in the text up to the cursor
	const CODE_BLOCK_REGEX = /```$/;

	/**
	 * Tests if the text matches the triple backtick pattern
	 */
	function testCodeBlockMatch(text: string): Match | null {
		const match = CODE_BLOCK_REGEX.exec(text);
		if (!match) {
			return null;
		}

		const start = match.index; // Position where ``` starts
		const end = text.length;

		return { start, end };
	}

	const editor = getEditor();

	function handleMatch(match: Match) {
		// Perform the transformation immediately when ``` is typed
		editor.update(() => {
			const selection = getSelection();
			if (!isRangeSelection(selection)) return;

			const anchor = selection.anchor;
			const node = anchor.getNode();
			if (!isTextNode(node)) return;

			const text = node.getTextContent();

			// Split the text: before the ```, and after the ```
			const beforeCode = text.substring(0, match.start);
			const afterCode = text.substring(match.end);

			// Update the current node to contain only the text before the code block
			node.setTextContent(beforeCode);

			// Create the code block (no language parameter)
			const codeNode = createCodeNode();

			// Insert structure: linebreak, code block, linebreak, text node (for continuation)
			let lastInserted = node.insertAfter(createLineBreakNode());

			if (lastInserted) {
				lastInserted = lastInserted.insertAfter(codeNode);
			}

			if (lastInserted) {
				lastInserted = lastInserted.insertAfter(createLineBreakNode());
			}

			// If there was text after the ```, put it on a new line after the code block
			if (afterCode.trim() && lastInserted) {
				const afterNode = createTextNode(afterCode);
				lastInserted.insertAfter(afterNode);
				// Focus inside the code block, not after
				codeNode.selectStart();
			} else {
				// Add an empty text node after the final linebreak so user can navigate down
				const emptyNode = createTextNode('');
				lastInserted?.insertAfter(emptyNode);
				// Focus inside the code block
				codeNode.selectStart();
			}
		});
	}

	function handleExit() {
		// No-op: transformation happens in handleMatch
	}

	/**
	 * Handle backspace/delete when cursor is in an empty code block.
	 * This allows users to delete empty code blocks easily.
	 */
	function handleDeletion(): boolean {
		const selection = getSelection();
		if (!isRangeSelection(selection)) return false;

		const anchor = selection.anchor;
		const node = anchor.getNode();
		if (!node || !(node instanceof CodeNode)) return false;

		// Only delete if the code block is empty
		const textContent = node.getTextContent();
		if (textContent.length > 0) return false;

		// Remove the code block
		node.remove();
		return true;
	}

	// Register deletion handlers
	$effect(() =>
		mergeUnlisten(
			editor.registerCommand(KEY_BACKSPACE_COMMAND, handleDeletion, COMMAND_PRIORITY_HIGH),
			editor.registerCommand(KEY_DELETE_COMMAND, handleDeletion, COMMAND_PRIORITY_HIGH)
		)
	);
</script>

<TypeAhead testMatch={testCodeBlockMatch} onMatch={handleMatch} onExit={handleExit} />

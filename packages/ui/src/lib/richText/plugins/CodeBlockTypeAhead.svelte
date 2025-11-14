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
		$createTextNode as createTextNode,
		$createParagraphNode as createParagraphNode,
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection,
		$isTextNode as isTextNode,
		$isParagraphNode as isParagraphNode,
		COMMAND_PRIORITY_HIGH,
		KEY_BACKSPACE_COMMAND,
		KEY_DELETE_COMMAND,
		KEY_ARROW_DOWN_COMMAND,
		KEY_ARROW_UP_COMMAND
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
		// Use queueMicrotask to ensure this runs after the current input event is processed
		// and added to the history, so undoing will revert to the state before typing ```
		editor.update(
			() => {
				const selection = getSelection();
				if (!isRangeSelection(selection)) return;

				const anchor = selection.anchor;
				const node = anchor.getNode();
				if (!isTextNode(node)) return;

				const paragraph = node.getParent();
				if (!isParagraphNode(paragraph)) return;

				const text = node.getTextContent();

				// Split the text: before the ```, and after the ```
				const beforeCode = text.substring(0, match.start);
				const afterCode = text.substring(match.end);

				// Create the code block paragraph
				const codeParagraph = createParagraphNode();
				const codeNode = createCodeNode();
				codeParagraph.append(codeNode);

				// Determine what to do based on whether we have text before/after
				const hasTextBefore = beforeCode.trim().length > 0;
				const hasTextAfter = afterCode.trim().length > 0;

				if (hasTextBefore) {
					// Keep the text before ``` in the current paragraph
					node.setTextContent(beforeCode);
					paragraph.insertAfter(codeParagraph);
				} else {
					// No text before, replace the current paragraph with the code block
					paragraph.replace(codeParagraph);
				}

				// Always create a paragraph after the code block
				if (hasTextAfter) {
					// Create a new paragraph with the text after ```
					const afterParagraph = createParagraphNode();
					afterParagraph.append(createTextNode(afterCode));
					codeParagraph.insertAfter(afterParagraph);
				} else {
					// Create an empty paragraph after the code block for navigation
					const emptyParagraph = createParagraphNode();
					emptyParagraph.append(createTextNode(''));
					codeParagraph.insertAfter(emptyParagraph);
				}

				// Focus inside the code block
				codeNode.selectStart();
			},
			{
				tag: 'history-merge'
			}
		);
	}

	function handleExit() {
		// No-op: transformation happens in handleMatch
	}

	/**
	 * Handle backspace when cursor is in an empty code block paragraph.
	 * This allows users to delete empty code blocks easily.
	 */
	function handleDeletion(): boolean {
		const selection = getSelection();
		if (!isRangeSelection(selection)) return false;

		const anchor = selection.anchor;
		const node = anchor.getNode();
		if (!node) return false;

		// Check if we're in a code block
		if (node instanceof CodeNode) {
			const textContent = node.getTextContent();
			if (textContent.length > 0) return false;

			const paragraph = node.getParent();
			if (!isParagraphNode(paragraph)) return false;

			// Remove the empty code block paragraph
			const prevParagraph = paragraph.getPreviousSibling();
			paragraph.remove();

			// Move cursor to end of previous paragraph, or stay put
			if (prevParagraph && isParagraphNode(prevParagraph)) {
				prevParagraph.selectEnd();
			}

			return true;
		}

		// Handle backspace at start of paragraph after empty code block
		if (isTextNode(node) && anchor.offset === 0) {
			const paragraph = node.getParent();
			if (!isParagraphNode(paragraph)) return false;

			const prevParagraph = paragraph.getPreviousSibling();
			if (!prevParagraph || !isParagraphNode(prevParagraph)) return false;

			// Check if previous paragraph is an empty code block
			const prevChild = prevParagraph.getFirstChild();
			if (prevChild instanceof CodeNode && prevChild.getTextContent().length === 0) {
				prevParagraph.remove();
				return true;
			}
		}

		return false;
	}

	/**
	 * Handle Arrow Down when cursor is at the end of a code block.
	 * This allows users to exit the code block and continue typing after it.
	 */
	function handleArrowDown(): boolean {
		const selection = getSelection();
		if (!isRangeSelection(selection)) return false;

		const anchor = selection.anchor;
		let node = anchor.getNode();
		if (!node) return false;

		// Check if we're in a CodeNode or if we're selecting the CodeNode itself
		let codeNode: CodeNode | null = null;

		if (node instanceof CodeNode) {
			codeNode = node;
		} else if (node.getParent() instanceof CodeNode) {
			// We're inside a text node within the code block
			codeNode = node.getParent() as CodeNode;
		}

		if (!codeNode) return false;

		// Check if we're at the end of the code block content
		const textContent = codeNode.getTextContent();
		const cursorOffset = anchor.offset;

		// Determine if we're on the last line
		const lastLineStart = textContent.lastIndexOf('\n') + 1;
		const isOnLastLine = cursorOffset >= lastLineStart;

		// Only exit if we're on the last line
		if (!isOnLastLine) return false;

		const codeParagraph = codeNode.getParent();
		if (!isParagraphNode(codeParagraph)) return false;

		// Find the next paragraph after the code block
		const nextParagraph = codeParagraph.getNextSibling();

		if (nextParagraph && isParagraphNode(nextParagraph)) {
			// Move to the start of the next paragraph
			nextParagraph.selectStart();
			return true;
		} else {
			// No next paragraph, create one
			const newParagraph = createParagraphNode();
			newParagraph.append(createTextNode(''));
			codeParagraph.insertAfter(newParagraph);
			newParagraph.selectStart();
			return true;
		}
	}

	/**
	 * Handle Arrow Up when cursor is at the start of a code block.
	 * This allows users to insert a paragraph before the code block.
	 */
	function handleArrowUp(e: KeyboardEvent): boolean {
		const selection = getSelection();
		if (!isRangeSelection(selection)) return false;

		const anchor = selection.anchor;
		let node = anchor.getNode();
		if (!node) return false;

		// Check if we're in a CodeNode or if we're selecting the CodeNode itself
		let codeNode: CodeNode | null = null;

		if (node instanceof CodeNode) {
			codeNode = node;
		} else if (node.getParent() instanceof CodeNode) {
			// We're inside a text node within the code block
			codeNode = node.getParent() as CodeNode;
		}

		if (!codeNode) return false;

		// Check if we're at the start of the code block content
		const cursorOffset = anchor.offset;

		// Only handle if we're at the very beginning (offset 0)
		if (cursorOffset !== 0) return false;

		const codeParagraph = codeNode.getParent();
		if (!isParagraphNode(codeParagraph)) return false;

		// Check if there's a previous paragraph
		const prevParagraph = codeParagraph.getPreviousSibling();

		if (!prevParagraph || !isParagraphNode(prevParagraph)) {
			// No previous paragraph, create one
			const newParagraph = createParagraphNode();
			newParagraph.append(createTextNode(''));
			codeParagraph.insertBefore(newParagraph);
			newParagraph.selectEnd();
			e.preventDefault();
		}
		return true;
	}

	// Register deletion and navigation handlers
	$effect(() =>
		mergeUnlisten(
			editor.registerCommand(KEY_BACKSPACE_COMMAND, handleDeletion, COMMAND_PRIORITY_HIGH),
			editor.registerCommand(KEY_DELETE_COMMAND, handleDeletion, COMMAND_PRIORITY_HIGH),
			editor.registerCommand(KEY_ARROW_DOWN_COMMAND, handleArrowDown, COMMAND_PRIORITY_HIGH),
			editor.registerCommand(KEY_ARROW_UP_COMMAND, handleArrowUp, COMMAND_PRIORITY_HIGH)
		)
	);
</script>

<TypeAhead testMatch={testCodeBlockMatch} onMatch={handleMatch} onExit={handleExit} />

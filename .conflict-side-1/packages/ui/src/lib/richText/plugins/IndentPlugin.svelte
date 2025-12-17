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

		// Handle both text nodes and paragraph nodes (for empty paragraphs)
		let parent;
		let textNode;
		let offset = anchor.offset;

		if (isTextNode(node)) {
			textNode = node;
			parent = node.getParent();
		} else if (isParagraphNode(node)) {
			// Selection is directly on paragraph (empty paragraph case)
			parent = node;
			textNode = parent.getFirstChild();
			// If paragraph has no text node, create one
			if (!textNode || !isTextNode(textNode)) {
				textNode = createTextNode('');
				parent.append(textNode);
				offset = 0;
			}
		} else {
			return false;
		}

		if (!isParagraphNode(parent)) return false;

		// Get the current paragraph text
		const currentLineText = parent.getTextContent();

		// Parse indentation and bullets from current line
		const indent = parseIndent(currentLineText);
		const bullet = parseBullet(currentLineText);

		// Check if we're in a continuation line (indented, no bullet)
		const isContinuationLine = !bullet && indent.length > 0;

		// If we're in a continuation line and at the end, we might want to create a new bullet
		// Check the previous sibling to see if it's part of a bullet list
		let shouldCreateNewBullet = false;
		let bulletToCreate: ReturnType<typeof parseBullet> = undefined;
		const textContent = textNode.getTextContent();

		if (isContinuationLine && offset === textContent.length) {
			// We're at the end of a continuation line - look backwards for the bullet
			let prevSibling = parent.getPreviousSibling();
			while (prevSibling && isParagraphNode(prevSibling)) {
				const prevText = prevSibling.getTextContent();
				const prevBullet = parseBullet(prevText);
				const prevIndent = parseIndent(prevText);

				if (prevBullet) {
					// Found a bullet - if its indent matches our indent, we should create a new bullet
					if (prevBullet.indent === indent) {
						shouldCreateNewBullet = true;
						bulletToCreate = prevBullet;
					}
					break;
				} else if (prevIndent !== indent) {
					// Different indentation means we're not part of the same list
					break;
				}

				prevSibling = prevSibling.getPreviousSibling();
			}
		}

		// Check if the next sibling is a continuation line (part of a wrapped paragraph)
		const nextSibling = parent.getNextSibling();
		const nextSiblingText =
			nextSibling && isParagraphNode(nextSibling) ? nextSibling.getTextContent() : '';
		const nextSiblingIndent = parseIndent(nextSiblingText);
		const nextSiblingBullet = parseBullet(nextSiblingText);

		// If we have a bullet and the next line is a continuation (indented but not a bullet),
		// then we're in the middle of a wrapped paragraph. The remainder should use continuation indent.
		const isNextLineContinuation =
			bullet && nextSiblingText && !nextSiblingBullet && nextSiblingIndent === bullet.indent;

		let newIndent = bullet ? bullet.prefix : indent;

		if (shouldCreateNewBullet && bulletToCreate) {
			// We're at the end of a continuation line - create a new bullet
			if (bulletToCreate.number) {
				const padding = bulletToCreate.prefix.length - bulletToCreate.prefix.trimStart().length;
				newIndent =
					bulletToCreate.prefix.substring(0, padding) + (bulletToCreate.number + 1) + '. ';
			} else {
				newIndent = bulletToCreate.prefix;
			}
		} else if (isNextLineContinuation) {
			// We're splitting a wrapped bullet - use continuation indent instead of creating new bullet
			newIndent = bullet.indent;
		} else if (bullet?.number) {
			// Parse and increment numeric bullet point
			const padding = bullet.prefix.length - bullet.prefix.trimStart().length;
			newIndent = bullet.prefix.substring(0, padding) + (bullet.number + 1) + '. ';
		}

		// Check if we're on an empty bullet line
		const trimmedLine = currentLineText.trim();
		if (bullet && trimmedLine === bullet.prefix.trim()) {
			// Clear the bullet from the current paragraph and keep cursor here
			const children = parent.getChildren();
			for (const child of children) {
				child.remove();
			}
			// Keep cursor in this paragraph (now empty, no longer a bullet)
			parent.select();
			return true;
		}

		// Split the paragraph at the cursor position
		const textAfterCursor = textContent.substring(offset);
		const textBeforeCursor = textContent.substring(0, offset);

		// Update current node's text to everything before cursor
		textNode.setTextContent(textBeforeCursor);

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

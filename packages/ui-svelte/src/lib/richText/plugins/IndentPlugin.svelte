<!--
@component
This component overrides enter key command to handle indentation and bullets in rich text mode.
-->
<script lang="ts" module>
	import type { TextNode, ParagraphNode, LexicalNode } from "lexical";

	/**
	 * If the cursor is on a non-simple text node (e.g. InlineCodeNode), adjust
	 * to an adjacent simple TextNode so the split moves the node whole.
	 *
	 * - Cursor at end → normalize to the start of the next simple sibling
	 *   (the node stays on the current line).
	 * - Cursor at start/middle → normalize to the end of the previous simple
	 *   sibling (the node moves to the new line).
	 */
	function normalizeNonSimpleTextNode(
		textNode: TextNode,
		offset: number,
	): { textNode: TextNode; offset: number } {
		if (textNode.isSimpleText()) return { textNode, offset };

		if (offset === textNode.getTextContent().length) {
			// Cursor at end — node stays on current line, split after it.
			const next = textNode.getNextSibling();
			if (next && isTextNode(next) && next.isSimpleText()) {
				return { textNode: next, offset: 0 };
			}
			const empty = createTextNode("");
			textNode.insertAfter(empty);
			return { textNode: empty, offset: 0 };
		}

		// Cursor at start/middle — node moves to new line, split before it.
		// Walk backwards past any adjacent non-simple nodes (e.g. consecutive
		// InlineCodeNodes) to find a simple TextNode we can safely split.
		let cursor = textNode.getPreviousSibling();
		while (cursor && isTextNode(cursor) && !cursor.isSimpleText()) {
			cursor = cursor.getPreviousSibling();
		}
		if (cursor && isTextNode(cursor)) {
			return { textNode: cursor, offset: cursor.getTextContent().length };
		}
		const empty = createTextNode("");
		textNode.insertBefore(empty);
		return { textNode: empty, offset: 0 };
	}

	/**
	 * Split a paragraph at the cursor position, moving text after the cursor
	 * and any subsequent sibling nodes into a new paragraph.
	 */
	function splitParagraph(
		textNode: TextNode,
		offset: number,
		parent: ParagraphNode,
		newIndent: string,
	): boolean {
		const textContent = textNode.getTextContent();
		let textBeforeCursor = textContent.substring(0, offset);
		let textAfterCursor = textContent.substring(offset);

		// Collect sibling nodes BEFORE modifying the text node — setTextContent
		// on certain node types (e.g. InlineCodeNode) can replace the node in
		// the tree, which would break the sibling chain.
		const siblingsToMove: LexicalNode[] = [];
		let sibling = textNode.getNextSibling();
		while (sibling) {
			siblingsToMove.push(sibling);
			sibling = sibling.getNextSibling();
		}

		// Discard separator whitespace at the split boundary when the cursor
		// is between text and sibling nodes (e.g. "Hello |`world`" — the space
		// is just a word separator and shouldn't appear on either line).
		if (siblingsToMove.length > 0 && textAfterCursor.trim() === "") {
			textBeforeCursor = textBeforeCursor.trimEnd();
			textAfterCursor = "";
		}

		textNode.setTextContent(textBeforeCursor);

		const newParagraph = createParagraphNode();
		const newTextNode = createTextNode(newIndent + textAfterCursor);
		newParagraph.append(newTextNode);

		for (const sib of siblingsToMove) {
			newParagraph.append(sib);
		}

		parent.insertAfter(newParagraph);

		const newSelection = createRangeSelection();
		newSelection.anchor.set(newTextNode.getKey(), newIndent.length, "text");
		newSelection.focus.set(newTextNode.getKey(), newIndent.length, "text");
		setSelection(newSelection);

		return true;
	}

	export function handleEnter(event: KeyboardEvent | null) {
		// Prevent the default browser behavior that creates extra paragraphs
		if (event) {
			event.preventDefault();
		}

		const selection = getSelection();
		if (!isRangeSelection(selection)) return false;

		const anchor = selection.anchor;
		const node = anchor.getNode();

		// If shift key is pressed, insert a plain newline with indentation but without bullet
		if (event?.shiftKey) {
			const rawNode = isTextNode(node) ? node : node.getFirstChild();
			const parent = isTextNode(node) ? node.getParent() : node;

			if (!isParagraphNode(parent) || !rawNode || !isTextNode(rawNode)) return false;

			const normalized = normalizeNonSimpleTextNode(rawNode, anchor.offset);

			const currentLineText = parent.getTextContent();
			const bullet = parseBullet(currentLineText);
			const indent = bullet ? bullet.indent : parseIndent(currentLineText);

			return splitParagraph(normalized.textNode, normalized.offset, parent, indent);
		}

		// Handle both text nodes and paragraph nodes (for empty paragraphs)
		let parent;
		let textNode;
		let offset = anchor.offset;

		if (isTextNode(node) && !node.isSimpleText()) {
			parent = node.getParent();
			({ textNode, offset } = normalizeNonSimpleTextNode(node, offset));
		} else if (isTextNode(node)) {
			textNode = node;
			parent = node.getParent();
		} else if (isParagraphNode(node)) {
			// Selection is directly on paragraph (empty paragraph case)
			parent = node;
			textNode = parent.getFirstChild();
			// If paragraph has no text node, create one
			if (!textNode || !isTextNode(textNode)) {
				textNode = createTextNode("");
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

		if (isContinuationLine && offset === textNode.getTextContent().length) {
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
			nextSibling && isParagraphNode(nextSibling) ? nextSibling.getTextContent() : "";
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
					bulletToCreate.prefix.substring(0, padding) + (bulletToCreate.number + 1) + ". ";
			} else {
				newIndent = bulletToCreate.prefix;
			}
		} else if (isNextLineContinuation) {
			// We're splitting a wrapped bullet - use continuation indent instead of creating new bullet
			newIndent = bullet.indent;
		} else if (bullet?.number) {
			// Parse and increment numeric bullet point
			const padding = bullet.prefix.length - bullet.prefix.trimStart().length;
			newIndent = bullet.prefix.substring(0, padding) + (bullet.number + 1) + ". ";
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

		return splitParagraph(textNode, offset, parent, newIndent);
	}
</script>

<script lang="ts">
	import { parseIndent, parseBullet } from "$lib/richText/linewrap";

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
		KEY_ENTER_COMMAND,
	} from "lexical";
	import { getEditor } from "svelte-lexical";

	const editor = getEditor();

	$effect(() => {
		const unregisterEnter = editor.registerCommand(
			KEY_ENTER_COMMAND,
			handleEnter,
			COMMAND_PRIORITY_CRITICAL,
		);

		return () => {
			unregisterEnter();
		};
	});
</script>

import {
	$isRangeSelection,
	$getSelection,
	TextNode,
	type LexicalEditor,
	type RangeSelection,
	$getRoot,
	$createParagraphNode,
	$createTextNode
} from 'lexical';
import { ImageNode } from 'svelte-lexical';

/**
 * Get the line text up to the caret position.
 */
export function getLineTextUpToAnchor(selection: RangeSelection): string | undefined {
	const anchor = selection.anchor;
	if (anchor.type !== 'text') {
		return undefined;
	}
	const anchorNode = anchor.getNode();
	if (!anchorNode.isSimpleText()) {
		return undefined;
	}
	const anchorOffset = anchor.offset;
	return anchorNode.getTextContent().slice(0, anchorOffset);
}

/**
 * Get the line text after the caret position.
 */
export function getLineTextAfterAnchor(selection: RangeSelection): string | undefined {
	const anchor = selection.anchor;
	if (anchor.type !== 'text') {
		return undefined;
	}
	const anchorNode = anchor.getNode();
	if (!anchorNode.isSimpleText()) {
		return undefined;
	}
	const anchorOffset = anchor.offset;
	return anchorNode.getTextContent().slice(anchorOffset);
}

/**
 * Get the text up to the caret position.
 */
export function getEditorTextUpToAnchor(selection: RangeSelection): string | undefined {
	const anchor = selection.anchor;

	if (anchor.type !== 'text') {
		return undefined;
	}

	const anchorNode = anchor.getNode();
	if (!anchorNode.isSimpleText()) {
		return undefined;
	}
	const buffer: string[] = [];
	const textNodes = $getRoot().getAllTextNodes();
	for (const node of textNodes) {
		if (anchor.key === node.getKey()) {
			break;
		}
		buffer.push(node.getTextContent());
	}
	const anchorOffset = anchor.offset;
	const anchorNodeText = anchorNode.getTextContent().slice(0, anchorOffset);
	buffer.push(anchorNodeText);
	return buffer.join('\n');
}

/**
 * Get the text after the caret position.
 */
export function getEditorTextAfterAnchor(selection: RangeSelection): string | undefined {
	const anchor = selection.anchor;

	if (anchor.type !== 'text') {
		return undefined;
	}

	const anchorNode = anchor.getNode();
	if (!anchorNode.isSimpleText()) {
		return undefined;
	}
	const buffer: string[] = [];
	const textNodes = $getRoot().getAllTextNodes();
	let found = false;
	for (const node of textNodes) {
		if (found) {
			buffer.push(node.getTextContent());
		}
		if (anchor.key === node.getKey()) {
			found = true;
		}
	}
	const anchorOffset = anchor.offset;
	const anchorNodeText = anchorNode.getTextContent().slice(anchorOffset);
	buffer.push(anchorNodeText);
	return buffer.join('\n');
}

export function getCursorPosition() {
	const nativeSelection = window.getSelection();
	const domRect = nativeSelection?.getRangeAt(0).getBoundingClientRect();

	if (domRect) {
		return { left: domRect.left + domRect.width, top: domRect.top };
	}
}

export function getSelectionPosition(windowScrollY?: number) {
	const nativeSelection = window.getSelection();
	const domRect = nativeSelection?.getRangeAt(0).getBoundingClientRect();

	if (domRect) {
		const top = domRect.top + (windowScrollY ?? 0);
		const left = domRect.left - 10;
		return { left, top };
	}
}

/**
 * Replace a section of text leading up to current caret
 * position. Note that we do not perform any checks here,
 * and assume the caller knows a valid selection, and
 * offset are passed.
 */
export function insertFilePath(editor: LexicalEditor, path: string, count: number) {
	editor.update(
		() => {
			const selection = $getSelection();
			if (!$isRangeSelection(selection)) {
				return;
			}

			const pathNode = new TextNode(path);

			const node = selection.getNodes().at(0);

			if (!(node instanceof TextNode)) {
				selection.insertNodes([pathNode]);
				return;
			}

			const offset = selection.anchor.offset;
			let target: TextNode | undefined;

			if (offset === count) {
				[target] = node.splitText(count);
			} else {
				[, target] = node.splitText(offset - count, offset);
			}

			if (!target) {
				throw new Error('Expected target');
			}

			target.replace(pathNode, false);
			pathNode.selectEnd();
		},
		{ tag: 'history-merge' }
	);
}

/**
 * Replace a section of text leading up to current caret
 * position. Note that we do not perform any checks here,
 * and assume the caller knows a valid selection, and
 * offset are passed.
 */
export function insertImageAtCaret(
	editor: LexicalEditor,
	image: { url: string; alt: string; count: number }
) {
	const { url, alt, count } = image;
	editor.update(() => {
		const selection = $getSelection();
		if (!$isRangeSelection(selection)) {
			return;
		}

		const imageNode = new ImageNode(url, alt, 300);

		const node = selection.getNodes().at(0);

		if (!(node instanceof TextNode)) {
			selection.insertNodes([imageNode]);
			return;
		}
		const offset = selection.anchor.offset;
		let target: TextNode | undefined;

		if (offset === count) {
			[target] = node.splitText(count);
		} else {
			[, target] = node.splitText(offset - count, offset);
		}
		target?.replace(imageNode, false);
	});
}

export function insertTextAtCaret(editor: LexicalEditor, text: string) {
	editor.update(
		() => {
			const selection = $getSelection();
			if (!$isRangeSelection(selection)) {
				return;
			}
			const node = selection.getNodes().at(0);

			if (!(node instanceof TextNode)) {
				selection.insertText(text);
				return;
			}
			const offset = selection.anchor.offset;
			node.spliceText(offset, 0, text);
		},
		{ tag: 'history-merge' }
	);
}

export function setEditorText(editor: LexicalEditor, text: string) {
	editor.update(
		() => {
			const root = $getRoot();
			root.clear();

			// Split text into lines and create a paragraph for each line
			// This preserves blank lines unlike markdown conversion
			const lines = text.split('\n');

			for (const line of lines) {
				const paragraph = $createParagraphNode();
				// Only create a text node if the line is not empty
				// Empty paragraphs represent blank lines
				if (line.length > 0) {
					const textNode = $createTextNode(line);
					paragraph.append(textNode);
				}
				root.append(paragraph);
			}
		},
		{ discrete: true }
	);
}

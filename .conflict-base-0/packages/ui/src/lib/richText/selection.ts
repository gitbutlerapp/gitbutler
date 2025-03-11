import { $isRangeSelection, $getSelection, TextNode, type LexicalEditor } from 'lexical';
import { ImageNode } from 'svelte-lexical';

export function getCursorPosition() {
	const nativeSelection = window.getSelection();
	const domRect = nativeSelection?.getRangeAt(0).getBoundingClientRect();

	if (domRect) {
		return { left: domRect.left + domRect.width, top: domRect.top + domRect.height };
	}
}

export function getSelectionPosition() {
	const nativeSelection = window.getSelection();
	const domRect = nativeSelection?.getRangeAt(0).getBoundingClientRect();

	if (domRect) {
		return { left: domRect.left - 10, top: domRect.top };
	}
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
		target?.replace(new ImageNode(url, alt, 300), false);
	});
}

export function insertTextAtCaret(editor: LexicalEditor, text: string) {
	editor.update(() => {
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
	});
}

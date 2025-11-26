import {
	$isTextNode as isTextNode,
	TextNode,
	$getSelection as getSelection,
	$isRangeSelection as isRangeSelection,
	type LexicalEditor,
	$getRoot,
	$isParagraphNode,
	ParagraphNode
} from 'lexical';

export type Bullet = {
	prefix: string;
	indent: string;
	number?: number;
};
type WrapArgs = {
	line: string;
	maxLength: number;
	remainder?: string;
	indent?: string;
	bullet?: Bullet;
};

/**
 * Helper function for (re)wrapping lines. Takes a line, and gives back a line
 * that fits within `maxLength`, and the remainder that should be carried over
 * to the next line.
 */
export function wrapLine({ line, maxLength, remainder = '', indent = '', bullet }: WrapArgs): {
	newLine: string;
	newRemainder: string;
} {
	// When we have a bullet, skip the bullet prefix to get the actual text parts
	const prefixLength = bullet ? bullet.prefix.length : indent.length;
	const parts = Array.from(line.substring(prefixLength).match(/([ \t]+|\S+)/g) || []);
	let acc = remainder.length > 0 ? indent + remainder + ' ' : bullet ? bullet.prefix : indent;

	for (let i = 0; i < parts.length; i++) {
		const word = parts[i];
		if (acc.length + word.length > maxLength) {
			// If acc is empty/just prefix, use the current word as newLine
			// and start remainder from next word
			if (!acc || acc === indent || acc === (bullet?.prefix ?? '')) {
				const remainingParts = parts.slice(i + 1);
				return {
					newLine: word,
					newRemainder: remainingParts.join('').trim()
				};
			}
			// Otherwise, acc becomes newLine and remainder starts from current word
			const remainingParts = parts.slice(i);
			return {
				newLine: acc,
				newRemainder: remainingParts.join('').trim()
			};
		}
		acc += word;
	}

	return { newLine: acc, newRemainder: '' };
}

export const WRAP_EXEMPTIONS = {
	FencedCodeBlock: /^ {0,3}(```.*$|~~~)/,
	HtmlBlock: /^[ \t]*<([a-zA-Z]+)(\s[^>]*)?>/,
	TableRow: /^\|/,
	BlockQuote: /^ *>/,
	Heading: /^ {0,3}#{1,6} /,
	HorizontalRule: /^ {0,3}(-{3,}|\*{3,}|_{3,})\s*$/,
	LinkedDefinition: /^\s*\[[^\]]+]:\s+\S+/,
	InlineLinkOrImage: /!?\[[^\]]*\]\([^)]*\)/
} as const;

type ExemptionId = keyof typeof WRAP_EXEMPTIONS;

export function isWrappingExempt(line: string): ExemptionId | undefined {
	const exemptions = Object.entries(WRAP_EXEMPTIONS) as [ExemptionId, RegExp][];
	return exemptions.find(([_, regex]) => regex.test(line))?.[0];
}

export function parseIndent(line: string) {
	return line.match(/^( +|\t+)/)?.[0] || '';
}

export function parseBullet(text: string): Bullet | undefined {
	const match = text.match(/^(\s*)([-*+]|(?<number>[0-9]+)\.)\s/);
	if (!match) return;
	const spaces = match[1] ?? '';
	const prefix = match[0];
	const numberStr = match.groups?.['number'];
	const number = numberStr ? parseInt(numberStr) : undefined;
	const indent = number ? ' '.repeat(number.toString().length + 2) : spaces + '  ';
	return { prefix, indent, number };
}

export function wrapIfNecessary({ node, maxLength }: { node: TextNode; maxLength: number }) {
	const line = node.getTextContent();
	if (line.length <= maxLength) {
		return;
	}
	if (line.indexOf(' ') === -1) {
		return; // No spaces to wrap at
	}
	if (isWrappingExempt(line)) {
		return; // Line contains text that should not be wrapped.
	}

	const bullet = parseBullet(line);
	const indent = bullet ? bullet.indent : parseIndent(line);
	const paragraph = node.getParent();

	if (!$isParagraphNode(paragraph)) {
		console.warn('[wrapIfNecessary] Node parent is not a paragraph:', paragraph?.getType());
		return;
	}

	const selection = getSelection();
	const selectionOffset = isRangeSelection(selection) ? selection.focus.offset : 0;

	// Wrap only the current line - don't collect other paragraphs
	const { newLine, newRemainder } = wrapLine({
		line,
		maxLength,
		indent,
		bullet
	});

	// Update current text node
	node.setTextContent(newLine);

	// If there's a remainder, create new paragraphs for it
	if (newRemainder) {
		let remainder = newRemainder;
		let lastParagraph = paragraph;

		// Create new paragraphs for the wrapped text
		while (remainder && remainder.length > 0) {
			// Prepend indent to the remainder before wrapping it
			const indentedLine = indent + remainder;
			const { newLine: finalLine, newRemainder: finalRem } = wrapLine({
				line: indentedLine,
				maxLength,
				indent
			});

			const newParagraph = new ParagraphNode();
			const newTextNode = new TextNode(finalLine);
			newParagraph.append(newTextNode);
			lastParagraph.insertAfter(newParagraph);

			lastParagraph = newParagraph;
			remainder = finalRem;
		}

		// Try to maintain cursor position
		// Calculate which paragraph the cursor should end up in
		let remainingOffset = selectionOffset;

		// If cursor was in the first line
		if (remainingOffset <= newLine.length) {
			// Keep cursor in the current paragraph at the same position
			node.select(remainingOffset, remainingOffset);
		} else {
			// Cursor should be in one of the wrapped paragraphs
			remainingOffset -= newLine.length + 1; // Account for the line and space

			// Walk through the created paragraphs to find where cursor belongs
			let currentPara: ParagraphNode | null = paragraph.getNextSibling() as ParagraphNode | null;
			let tempRemainder = newRemainder;

			// Calculate all the wrapped lines to find cursor position
			while (tempRemainder && tempRemainder.length > 0) {
				const indentedLine = indent + tempRemainder;
				const { newLine: tempLine, newRemainder: tempRem } = wrapLine({
					line: indentedLine,
					maxLength,
					indent
				});

				// tempLine now includes the indent, so just check against its length
				if (remainingOffset <= tempLine.length) {
					// Cursor belongs in this line
					break;
				}
				remainingOffset -= tempLine.length + 1; // +1 for space between lines
				tempRemainder = tempRem;
				currentPara = currentPara?.getNextSibling() as ParagraphNode | null;
			}

			// Set cursor in the appropriate paragraph
			if (currentPara && $isParagraphNode(currentPara)) {
				const textNode = currentPara.getFirstChild();
				if (isTextNode(textNode)) {
					textNode.select(Math.max(0, remainingOffset), Math.max(0, remainingOffset));
				}
			} else {
				// Fallback: put cursor at end of last created paragraph
				if (lastParagraph) {
					const textNode = lastParagraph.getFirstChild();
					if (isTextNode(textNode)) {
						textNode.selectEnd();
					}
				}
			}
		}
	}
}

export function wrapAll(editor: LexicalEditor, maxLength: number) {
	editor.update(
		() => {
			const root = $getRoot();
			const children = root.getChildren();

			for (const child of children) {
				if ($isParagraphNode(child)) {
					const textNode = child.getFirstChild();
					if (isTextNode(textNode)) {
						wrapIfNecessary({ node: textNode, maxLength });
					}
				}
			}
		},
		{ tag: 'history-merge' }
	);
}

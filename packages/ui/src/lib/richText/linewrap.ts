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
	const parts = Array.from(line.substring(indent.length).match(/([ \t]+|\S+)/g) || []);
	let acc = remainder.length > 0 ? indent + remainder + ' ' : bullet ? bullet.prefix : indent;
	for (const word of parts) {
		if (acc.length + word.length > maxLength) {
			const newLine = acc ? acc : word;
			const concatLine = remainder
				? indent + remainder + ' ' + line.substring(indent.length)
				: line;
			const newRemainder = concatLine.slice(newLine.length).trim();

			return {
				newLine,
				newRemainder
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

export function wrapIfNecssary({ node, maxLength }: { node: TextNode; maxLength: number }) {
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
		console.warn('[wrapIfNecssary] Node parent is not a paragraph:', paragraph?.getType());
		return;
	}

	const selection = getSelection();
	const selectionOffset = isRangeSelection(selection) ? selection.focus.offset : 0;

	// Wrap the current line
	const { newLine, newRemainder } = wrapLine({
		line,
		maxLength,
		indent,
		bullet
	});

	// Update current text node
	node.setTextContent(newLine);

	// If there's a remainder, we need to create new paragraphs or reuse related ones
	if (newRemainder) {
		let remainder = newRemainder;
		let lastParagraph = paragraph;

		// Get related paragraphs (paragraphs with same indentation following this one)
		const relatedParagraphs = getRelatedParagraphs(paragraph, indent);

		// Process the remainder with related paragraphs
		for (const relatedPara of relatedParagraphs) {
			if (!remainder) break;

			const relatedText = relatedPara.getTextContent();

			// Combine remainder with related paragraph text
			const combinedText = remainder + ' ' + relatedText;
			const { newLine: wrappedLine, newRemainder: newRem } = wrapLine({
				line: combinedText,
				maxLength,
				indent
			});

			// Update the related paragraph
			const textNode = relatedPara.getFirstChild();
			if (isTextNode(textNode)) {
				textNode.setTextContent(wrappedLine);
			}

			remainder = newRem;
			lastParagraph = relatedPara;
		}

		// Create new paragraphs for any remaining text
		while (remainder && remainder.length > 0) {
			const { newLine: finalLine, newRemainder: finalRem } = wrapLine({
				line: remainder,
				maxLength,
				indent
			});

			const newParagraph = new ParagraphNode();
			const newTextNode = new TextNode(indent + finalLine);
			newParagraph.append(newTextNode);
			lastParagraph.insertAfter(newParagraph);

			lastParagraph = newParagraph;
			remainder = finalRem;
		}

		// Try to maintain cursor position
		if (selectionOffset > newLine.length) {
			// Cursor was after the wrap point
			// Try to find which paragraph it should be in now
			let targetPara = paragraph;
			let accumulatedLength = newLine.length;

			// Walk through paragraphs to find where cursor should land
			let nextPara = paragraph.getNextSibling();
			while (nextPara && $isParagraphNode(nextPara)) {
				const nextText = nextPara.getFirstChild();
				if (!isTextNode(nextText)) break;

				const paraLength = nextText.getTextContentSize();

				// Check if cursor should be in this paragraph
				if (selectionOffset <= accumulatedLength + paraLength) {
					const offset = Math.min(selectionOffset - accumulatedLength, paraLength);
					nextText.select(offset, offset);
					return;
				}

				accumulatedLength += paraLength;
				targetPara = nextPara;
				nextPara = nextPara.getNextSibling();

				// Stop at non-related paragraphs
				const text = targetPara.getTextContent();
				const paraIndent = parseIndent(text);
				if (paraIndent !== indent || parseBullet(text)) {
					break;
				}
			}

			// If we didn't find a place, put cursor at end of last paragraph
			if (targetPara) {
				const lastText = targetPara.getFirstChild();
				if (isTextNode(lastText)) {
					lastText.selectEnd();
				}
			}
		}
	}
}

/**
 * Returns paragraphs that follow the given paragraph that are considered part of the same
 * logical paragraph. This enables us to re-wrap a paragraph when edited in the middle.
 *
 * In the multi-paragraph structure, "related paragraphs" are those with the same
 * indentation and no bullet points, representing continuation of the same text block.
 */
function getRelatedParagraphs(paragraph: ParagraphNode, indent: string): ParagraphNode[] {
	const collectedParagraphs: ParagraphNode[] = [];
	let next = paragraph.getNextSibling();

	while (next && $isParagraphNode(next)) {
		const text = next.getTextContent();

		// Empty paragraphs break the chain
		if (text.trimStart() === '') {
			break;
		}

		// We don't consider altered indentations or new bullet points to be
		// part of the same logical paragraph.
		const bullet = parseBullet(text);
		const lineIndent = parseIndent(text);

		if (indent !== lineIndent || bullet) {
			break;
		}

		collectedParagraphs.push(next);
		next = next.getNextSibling();
	}

	return collectedParagraphs;
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
						wrapIfNecssary({ node: textNode, maxLength });
					}
				}
			}
		},
		{ tag: 'history-merge' }
	);
}

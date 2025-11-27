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
		const nextAcc = acc + word;
		// Check if adding this word (including trailing spaces in acc) would exceed maxLength
		// We need to check the trimmed length to avoid counting trailing spaces
		if (nextAcc.trimEnd().length > maxLength) {
			// If acc is empty/just prefix, use the current word as newLine
			// and start remainder from next word
			const accTrimmed = acc.trimEnd();
			if (
				!accTrimmed ||
				accTrimmed === indent.trimEnd() ||
				accTrimmed === (bullet?.prefix.trimEnd() ?? '')
			) {
				const remainingParts = parts.slice(i + 1);
				return {
					newLine: word,
					newRemainder: remainingParts.join('').trim()
				};
			}
			// Otherwise, acc becomes newLine and remainder starts from current word
			const remainingParts = parts.slice(i);
			return {
				newLine: accTrimmed,
				newRemainder: remainingParts.join('').trim()
			};
		}
		acc = nextAcc;
	}

	return { newLine: acc.trimEnd(), newRemainder: '' };
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

/**
 * Checks if a paragraph is the start of a new logical paragraph.
 * A new logical paragraph begins when:
 * - The line is empty
 * - The line starts with a bullet point
 * - The line has different indentation
 * - The line is wrapping-exempt (code blocks, headings, etc.)
 */
function isLogicalParagraphBoundary(para: ParagraphNode, previousIndent: string): boolean {
	const text = para.getTextContent();

	if (!text.trim()) return true; // Empty line
	if (parseBullet(text)) return true; // Bullet point
	if (isWrappingExempt(text)) return true; // Code blocks, headings, etc.
	if (parseIndent(text) !== previousIndent) return true; // Different indentation

	return false;
}

/**
 * Collects all paragraphs that belong to the same logical paragraph.
 * NOTE: This function only collects paragraphs FORWARD (nextSibling), never backward.
 * Empty paragraphs are never collected as they are considered logical boundaries.
 */
function collectLogicalParagraph(paragraph: ParagraphNode, indent: string): ParagraphNode[] {
	const paragraphs: ParagraphNode[] = [paragraph];
	let nextSibling = paragraph.getNextSibling();

	while (nextSibling && $isParagraphNode(nextSibling)) {
		// Extra defensive check: never collect empty paragraphs
		const siblingText = nextSibling.getTextContent();
		if (siblingText.trim() === '') break;

		if (isLogicalParagraphBoundary(nextSibling, indent)) break;
		paragraphs.push(nextSibling);
		nextSibling = nextSibling.getNextSibling();
	}

	return paragraphs;
}

/**
 * Combines text from all paragraphs in a logical paragraph.
 */
function combineLogicalParagraphText(
	paragraphs: ParagraphNode[],
	indent: string,
	firstLineText: string
): string {
	let combined = firstLineText;

	for (let i = 1; i < paragraphs.length; i++) {
		const text = paragraphs[i].getTextContent();
		const textWithoutIndent = text.startsWith(indent) ? text.substring(indent.length) : text;
		combined += ' ' + textWithoutIndent;
	}

	return combined;
}

/**
 * Wraps combined text into multiple lines respecting maxLength.
 */
function wrapCombinedText(
	combinedText: string,
	maxLength: number,
	indent: string,
	bullet: Bullet | undefined
): string[] {
	const wrappedLines: string[] = [];
	let remainder = combinedText;
	let isFirstLine = true;

	while (remainder.length > 0) {
		const lineToWrap = isFirstLine ? remainder : indent + remainder;
		const { newLine, newRemainder } = wrapLine({
			line: lineToWrap,
			maxLength,
			indent: isFirstLine ? '' : indent,
			bullet: isFirstLine ? bullet : undefined
		});

		wrappedLines.push(newLine);
		remainder = newRemainder;
		isFirstLine = false;
	}

	return wrappedLines;
}

/**
 * Updates the DOM by replacing old paragraphs with wrapped lines.
 */
function updateParagraphsWithWrappedLines(
	paragraph: ParagraphNode,
	paragraphsToRemove: ParagraphNode[],
	wrappedLines: string[]
): void {
	// Remove old continuation paragraphs
	for (let i = 1; i < paragraphsToRemove.length; i++) {
		paragraphsToRemove[i].remove();
	}

	// Update the first paragraph with the first wrapped line
	const children = paragraph.getChildren();
	const firstTextNode = children.find((child) => isTextNode(child)) as TextNode | undefined;

	if (firstTextNode) {
		firstTextNode.setTextContent(wrappedLines[0]);
		// Remove all other children
		children.forEach((child) => {
			if (child !== firstTextNode) child.remove();
		});
	} else {
		// Fallback: no text nodes found, create one
		paragraph.append(new TextNode(wrappedLines[0]));
	}

	// Create new paragraphs for additional wrapped lines
	let lastParagraph = paragraph;
	for (let i = 1; i < wrappedLines.length; i++) {
		const newParagraph = new ParagraphNode();
		newParagraph.append(new TextNode(wrappedLines[i]));
		lastParagraph.insertAfter(newParagraph);
		lastParagraph = newParagraph;
	}
}

/**
 * Repositions the cursor to the appropriate location after wrapping.
 */
function repositionCursor(
	paragraph: ParagraphNode,
	wrappedLines: string[],
	selectionOffset: number
): void {
	const firstTextNode = paragraph.getFirstChild();
	if (!isTextNode(firstTextNode)) return;

	let remainingOffset = selectionOffset;
	let targetLineIndex = 0;

	// Find which line the cursor should be on
	for (let i = 0; i < wrappedLines.length; i++) {
		if (remainingOffset <= wrappedLines[i].length) {
			targetLineIndex = i;
			break;
		}
		remainingOffset -= wrappedLines[i].length + 1; // +1 for space between lines
	}

	// Set cursor in the appropriate paragraph
	if (targetLineIndex === 0) {
		firstTextNode.select(Math.max(0, remainingOffset), Math.max(0, remainingOffset));
		return;
	}

	// Navigate to the target paragraph
	let currentPara: ParagraphNode | null = paragraph.getNextSibling() as ParagraphNode | null;
	for (let i = 1; i < targetLineIndex && currentPara; i++) {
		currentPara = currentPara.getNextSibling() as ParagraphNode | null;
	}

	if (currentPara && $isParagraphNode(currentPara)) {
		const textNode = currentPara.getFirstChild();
		if (isTextNode(textNode)) {
			textNode.select(Math.max(0, remainingOffset), Math.max(0, remainingOffset));
		}
	}
}

export function wrapIfNecessary({ node, maxLength }: { node: TextNode; maxLength: number }) {
	const paragraph = node.getParent();

	if (!$isParagraphNode(paragraph)) {
		console.warn('[wrapIfNecessary] Node parent is not a paragraph:', paragraph?.getType());
		return;
	}

	const line = paragraph.getTextContent();

	// Early returns for cases where wrapping isn't needed
	if (line.length <= maxLength || !line.includes(' ') || isWrappingExempt(line)) {
		return;
	}

	const bullet = parseBullet(line);
	const indent = bullet ? bullet.indent : parseIndent(line);
	const selection = getSelection();
	const selectionOffset = isRangeSelection(selection) ? selection.focus.offset : 0;

	// Collect, combine, and wrap the logical paragraph
	const paragraphsToRewrap = collectLogicalParagraph(paragraph, indent);
	const combinedText = combineLogicalParagraphText(paragraphsToRewrap, indent, line);
	const wrappedLines = wrapCombinedText(combinedText, maxLength, indent, bullet);

	// Update the DOM with wrapped lines
	updateParagraphsWithWrappedLines(paragraph, paragraphsToRewrap, wrappedLines);

	// Restore cursor position
	repositionCursor(paragraph, wrappedLines, selectionOffset);
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

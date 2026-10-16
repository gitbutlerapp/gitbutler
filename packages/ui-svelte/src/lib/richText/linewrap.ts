import {
	$isTextNode as isTextNode,
	TextNode,
	$getSelection as getSelection,
	$isRangeSelection as isRangeSelection,
	type LexicalEditor,
	type LexicalNode,
	$getRoot,
	$isParagraphNode,
	ParagraphNode,
} from "lexical";

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
export function wrapLine({ line, maxLength, remainder = "", indent = "", bullet }: WrapArgs): {
	newLine: string;
	newRemainder: string;
} {
	// When we have a bullet, skip the bullet prefix to get the actual text parts
	const prefixLength = bullet ? bullet.prefix.length : indent.length;
	const parts = Array.from(line.substring(prefixLength).match(/([ \t]+|\S+)/g) || []);
	let acc = remainder.length > 0 ? indent + remainder + " " : bullet ? bullet.prefix : indent;

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
				accTrimmed === (bullet?.prefix.trimEnd() ?? "")
			) {
				const remainingParts = parts.slice(i + 1);
				return {
					newLine: word,
					newRemainder: remainingParts.join("").trim(),
				};
			}
			// Otherwise, acc becomes newLine and remainder starts from current word
			const remainingParts = parts.slice(i);
			return {
				newLine: accTrimmed,
				newRemainder: remainingParts.join("").trim(),
			};
		}
		acc = nextAcc;
	}

	// Check if we have trailing whitespace that would push us over maxLength
	const trimmedAcc = acc.trimEnd();
	if (trimmedAcc.length === maxLength && acc.length > maxLength) {
		// We're exactly at max length with trailing space(s)
		// Return a space as remainder to create an empty next line
		return { newLine: trimmedAcc, newRemainder: " " };
	}

	return { newLine: trimmedAcc, newRemainder: "" };
}

export const WRAP_EXEMPTIONS = {
	FencedCodeBlock: /^ {0,3}(```.*$|~~~)/,
	HtmlBlock: /^[ \t]*<([a-zA-Z]+)(\s[^>]*)?>/,
	TableRow: /^\|/,
	BlockQuote: /^ *>/,
	Heading: /^ {0,3}#{1,6} /,
	HorizontalRule: /^ {0,3}(-{3,}|\*{3,}|_{3,})\s*$/,
	LinkedDefinition: /^\s*\[[^\]]+]:\s+\S+/,
	InlineLinkOrImage: /!?\[[^\]]*\]\([^)]*\)/,
} as const;

type ExemptionId = keyof typeof WRAP_EXEMPTIONS;

export function isWrappingExempt(line: string): ExemptionId | undefined {
	const exemptions = Object.entries(WRAP_EXEMPTIONS) as [ExemptionId, RegExp][];
	return exemptions.find(([_, regex]) => regex.test(line))?.[0];
}

export function parseIndent(line: string) {
	return line.match(/^[ \t]+/)?.[0] || "";
}

export function parseBullet(text: string): Bullet | undefined {
	const match = text.match(/^(\s*)([-*+]|(?<number>[0-9]+)\.)\s/);
	if (!match) return;
	const spaces = match[1] ?? "";
	const prefix = match[0];
	const numberStr = match.groups?.["number"];
	const number = numberStr ? parseInt(numberStr) : undefined;
	const indent = number ? " ".repeat(number.toString().length + 2) : spaces + "  ";
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
		if (siblingText.trim() === "") break;

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
	firstLineText: string,
): string {
	let combined = firstLineText;

	for (let i = 1; i < paragraphs.length; i++) {
		const text = paragraphs[i].getTextContent();
		const textWithoutIndent = text.startsWith(indent) ? text.substring(indent.length) : text;
		combined += " " + textWithoutIndent;
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
	bullet: Bullet | undefined,
): string[] {
	const wrappedLines: string[] = [];
	let remainder = combinedText;
	let isFirstLine = true;

	while (remainder.length > 0) {
		const lineToWrap = isFirstLine ? remainder : indent + remainder;
		const { newLine, newRemainder } = wrapLine({
			line: lineToWrap,
			maxLength,
			indent: isFirstLine ? "" : indent,
			bullet: isFirstLine ? bullet : undefined,
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
	wrappedLines: string[],
): void {
	// Remove old continuation paragraphs
	for (let i = 1; i < paragraphsToRemove.length; i++) {
		paragraphsToRemove[i].remove();
	}

	// Update the first paragraph with the first wrapped line
	const children = paragraph.getChildren();
	const firstTextNode = children.find((child) => isTextNode(child));

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
 * Computes the global text offset within a paragraph for the current selection.
 * This is needed because `selection.focus.offset` is relative to the focused
 * text node, but when a paragraph contains multiple nodes (e.g. inline code
 * from backticks), we need the offset relative to the entire paragraph text.
 */
function getGlobalSelectionOffset(paragraph: ParagraphNode): number {
	const selection = getSelection();
	if (!isRangeSelection(selection)) return 0;

	const focusNode = selection.focus.getNode();
	const focusOffset = selection.focus.offset;

	// Collect all text nodes in DFS order
	const textNodes: TextNode[] = [];
	collectTextNodes(paragraph, textNodes);

	// When focus type is "element", the offset is a child index, not a character
	// offset. Convert it by summing text content sizes of children up to that index.
	if (selection.focus.type === "element") {
		if (focusNode.getKey() !== paragraph.getKey()) return 0;
		let charOffset = 0;
		for (let i = 0; i < Math.min(focusOffset, textNodes.length); i++) {
			charOffset += textNodes[i].getTextContentSize();
		}
		return charOffset;
	}

	// Focus type is "text" — find the text node and sum preceding lengths
	let offset = 0;
	for (const textNode of textNodes) {
		if (textNode.getKey() === focusNode.getKey()) {
			return offset + focusOffset;
		}
		offset += textNode.getTextContentSize();
	}

	// Focus node not found in this paragraph — don't guess
	return 0;
}

function collectTextNodes(node: LexicalNode, result: TextNode[]): void {
	if (isTextNode(node)) {
		result.push(node);
		return;
	}
	if ("getChildren" in node && typeof node.getChildren === "function") {
		for (const child of node.getChildren() as LexicalNode[]) {
			collectTextNodes(child, result);
		}
	}
}

/**
 * Repositions the cursor to the appropriate location after wrapping.
 */
function repositionCursor(
	paragraph: ParagraphNode,
	wrappedLines: string[],
	selectionOffset: number,
	indent: string,
): void {
	const firstTextNode = paragraph.getFirstChild();
	if (!isTextNode(firstTextNode)) return;

	// Build a map of combined text position to (lineIndex, offsetInLine)
	// The wrapped lines contain the actual text with indentation added to continuation lines
	let combinedLength = 0;
	let targetLineIndex = wrappedLines.length - 1; // Default to last line
	let offsetInTargetLine = wrappedLines[targetLineIndex].length; // Default to end

	// Find which line the cursor should be on
	for (let i = 0; i < wrappedLines.length; i++) {
		const lineLength = wrappedLines[i].length;
		// Continuation lines have indent added, so we need to account for the text without indent
		const indentLength = i > 0 ? indent.length : 0;
		const textLengthWithoutIndent = lineLength - indentLength;

		// Check if the selection offset falls within this line's range
		if (selectionOffset <= combinedLength + textLengthWithoutIndent) {
			targetLineIndex = i;
			// For continuation lines, add back the indent length
			offsetInTargetLine = selectionOffset - combinedLength + indentLength;
			break;
		}

		// Move to next line: add the text length (without indent) + 1 for the space that was between lines
		combinedLength += textLengthWithoutIndent + 1;
	}

	// Set cursor in the appropriate paragraph
	if (targetLineIndex === 0) {
		firstTextNode.select(Math.max(0, offsetInTargetLine), Math.max(0, offsetInTargetLine));
		return;
	}

	// Navigate to the target paragraph
	let currentPara: ParagraphNode | null = paragraph.getNextSibling();
	for (let i = 1; i < targetLineIndex && currentPara; i++) {
		currentPara = currentPara.getNextSibling();
	}

	if (currentPara && $isParagraphNode(currentPara)) {
		const textNode = currentPara.getFirstChild();
		if (isTextNode(textNode)) {
			textNode.select(Math.max(0, offsetInTargetLine), Math.max(0, offsetInTargetLine));
		}
	}
}

export function wrapIfNecessary({ node, maxLength }: { node: TextNode; maxLength: number }) {
	const paragraph = node.getParent();

	if (!$isParagraphNode(paragraph)) {
		console.warn("[wrapIfNecessary] Node parent is not a paragraph:", paragraph?.getType());
		return;
	}

	const line = paragraph.getTextContent();

	// Early returns for cases where wrapping isn't needed
	if (line.length <= maxLength || !line.includes(" ") || isWrappingExempt(line)) {
		return;
	}

	const bullet = parseBullet(line);
	const indent = bullet ? bullet.indent : parseIndent(line);
	const selectionOffset = getGlobalSelectionOffset(paragraph);

	// Collect, combine, and wrap the logical paragraph
	const paragraphsToRewrap = collectLogicalParagraph(paragraph, indent);
	const combinedText = combineLogicalParagraphText(paragraphsToRewrap, indent, line);
	const wrappedLines = wrapCombinedText(combinedText, maxLength, indent, bullet);

	// Update the DOM with wrapped lines
	updateParagraphsWithWrappedLines(paragraph, paragraphsToRewrap, wrappedLines);

	// Restore cursor position
	repositionCursor(paragraph, wrappedLines, selectionOffset, indent);
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
		{ tag: "history-merge" },
	);
}

import {
	$isTextNode as isTextNode,
	$isLineBreakNode as isLineBreakNode,
	TextNode,
	LineBreakNode,
	$createRangeSelection as createRangeSelection,
	$setSelection as setSelection,
	$getSelection as getSelection,
	$isRangeSelection as isRangeSelection,
	type LexicalNode,
	type LexicalEditor,
	$getRoot,
	$isParagraphNode
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
	const bullet = parseBullet(line);
	const indent = bullet ? bullet.indent : parseIndent(line);
	const selection = getSelection();

	if (line.length <= maxLength || line.indexOf(' ') === -1) {
		return; // Line does not exceed max length.
	}
	if (isWrappingExempt(line)) {
		return; // Line contains text that should not be wrapped.
	}

	/** Number of characters into the next line that the cursor should be moved. */
	let selectionCarry: number | undefined = undefined;

	// An array for collecting all modified and inserted text.
	const insertedNodes: TextNode[] = [];

	// Remainder string that should be carried over between lines when
	// re-wrapping lines.
	let remainder = '';

	// We want to consider the modified line, and the remaining lines from
	// the same pagraph.
	const relatedNodes = getRelatedLines(node, indent);

	const { newLine, newRemainder } = wrapLine({
		line,
		remainder,
		maxLength,
		indent,
		bullet
	});

	const newNode = new TextNode(newLine);
	node.replace(newNode);
	insertedNodes.push(newNode);
	remainder = newRemainder;

	// Check if selection should be wrapped to the next line.
	const selectionOffset = isRangeSelection(selection) ? selection.focus.offset : 0;
	const cursorDiff = selectionOffset - newLine.length;
	// Number of characters carried over from last row.
	selectionCarry = cursorDiff > 0 ? cursorDiff : undefined;

	// If white space carries over then we insert a new line.
	if (remainder === '' && selectionCarry !== undefined) {
		const newKey = newNode
			.insertAfter(new LineBreakNode())!
			.insertAfter(new TextNode(''))!
			.getKey();
		moveCursorTo(newKey, indent.length);
		return;
	}

	// Carry over possible remainder and re-wrap the rest of paragraph.
	for (const value of relatedNodes) {
		const line = value.getTextContent();
		const { newLine, newRemainder } = wrapLine({ line, remainder, maxLength, indent });

		const newNode = new TextNode(newLine);
		value.replace(newNode);

		insertedNodes.push(newNode);
		remainder = newRemainder;
	}

	// Insert any final remainder at the end of the paragraph.
	if (remainder) {
		while (remainder.length > 0) {
			const { newLine, newRemainder } = wrapLine({ line: remainder, maxLength });
			const newNode = new TextNode(indent + newLine);
			insertedNodes.at(-1)!.insertAfter(new LineBreakNode(), true)!.insertAfter(newNode, true);
			insertedNodes.push(newNode);
			remainder = newRemainder;
		}
	}

	if (selectionCarry !== undefined) {
		// In a simplified world the cursor does not move at all, or it
		// gets shifted to the next line. Successive lines can still be
		// reformatted, but the cursor should never move there.
		const secondNode = insertedNodes.at(1);

		if (secondNode) {
			moveCursorTo(secondNode.getKey(), indent.length + selectionCarry);
		}
	}
}

/**
 * Returns nodes that follow the given node that are considered part of the same
 * paragraph. This enables us to re-wrap a paragraph when edited in the middle.
 */
function getRelatedLines(node: TextNode, indent: string): TextNode[] {
	// Iterator for finding the rest of the paragraph.
	let n: LexicalNode | null = node;

	// Get the first sibling node.
	n = n.getNextSibling()?.getNextSibling() || null;
	if (!n || !isTextNode(n)) {
		return [];
	}

	const collectedNodes: TextNode[] = [];

	while (n) {
		const line = n.getTextContent();
		if (!isTextNode(n) || line.trimStart() === '') {
			break;
		}

		// We don't consider altered indentations or new bullet points to be
		// part of the same paragraph.
		const bullet = parseBullet(line);
		const lineIndent = parseIndent(line);
		if (indent !== lineIndent || bullet) {
			break;
		}

		collectedNodes.push(n);
		n = n.getNextSibling();
		if (!n) {
			break;
		} else if (!isLineBreakNode(n)) {
			throw new Error('Expected line break node');
		}
		n = n.getNextSibling();
	}

	return collectedNodes;
}

function moveCursorTo(nodeKey: string, position: number) {
	const selection = createRangeSelection();
	selection.anchor.set(nodeKey, position, 'text');
	selection.focus.set(nodeKey, position, 'text');
	setSelection(selection);
}

export function wrapAll(editor: LexicalEditor, maxLength: number) {
	editor.update(() => {
		const paragraph = $getRoot().getFirstChild();
		if ($isParagraphNode(paragraph)) {
			let node = paragraph.getFirstChild();
			while (node) {
				if (isTextNode(node)) {
					wrapIfNecssary({ node, maxLength });
				}
				node = node.getNextSibling();
			}
		}
	});
}

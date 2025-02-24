import {
	TextNode,
	type BaseSelection,
	type EditorConfig,
	type LexicalNode,
	type NodeKey,
	type SerializedTextNode,
	type Spread,
	$isRangeSelection as isRangeSelection,
	$isTextNode as isTextNode,
	$createTextNode as createTextNode
} from 'lexical';

export type InlineCodeMatch = {
	end: number;
	start: number;
	code: string;
};

export type SerializedInlineCodeNode = Spread<
	{
		type: 'inline-code';
		code: string;
	},
	SerializedTextNode
>;

export class InlineCodeNode extends TextNode {
	__code: string;

	static getType(): string {
		return 'inline-code';
	}

	static clone(node: InlineCodeNode): InlineCodeNode {
		return new InlineCodeNode(node.__code, node.__key);
	}

	constructor(code: string, key?: NodeKey) {
		super(code, key);
		this.__code = code;
	}

	createDOM(_config: EditorConfig): HTMLElement {
		const dom = document.createElement('code');
		dom.className = 'inline-code';
		dom.textContent = this.__code;
		return dom;
	}

	static importJSON(serializedNode: SerializedInlineCodeNode): InlineCodeNode {
		const node = createInlineCodeNode(serializedNode.code);
		node.setFormat(serializedNode.format);
		node.setDetail(serializedNode.detail);
		node.setMode(serializedNode.mode);
		node.setStyle(serializedNode.style);
		return node;
	}

	exportJSON(): SerializedInlineCodeNode {
		return {
			...super.exportJSON(),
			type: 'inline-code',
			code: this.__code
		};
	}

	updateDOM(prevNode: this, dom: HTMLElement, _config: EditorConfig): boolean {
		if (prevNode.__code !== this.__code) {
			dom.textContent = this.__code;
		}
		return false;
	}

	canInsertTextBefore(): boolean {
		return false;
	}

	canInsertTextAfter(): boolean {
		return false;
	}

	isTextEntity(): boolean {
		return true;
	}

	getTextContent(): string {
		return this.__code;
	}

	setTextContent(text: string): this {
		// If the text no longer has backticks on both ends, convert back to a regular text node
		if (!text.startsWith('`') || !text.endsWith('`') || text.length < 2) {
			const textNode = createTextNode(text);
			this.replace(textNode);
			return textNode as any;
		}

		const writable = this.getWritable();
		writable.__code = text;
		writable.__text = text;
		return writable;
	}
}

export function createInlineCodeNode(code: string): InlineCodeNode {
	return new InlineCodeNode(code);
}

export function isInlineCodeNode(node: LexicalNode): node is InlineCodeNode {
	return node instanceof InlineCodeNode;
}

function getTextSurroundingCode(text: string, start: number, end: number): [string, string] {
	const before = text.slice(0, start);
	let after = text.slice(end);
	after = after.startsWith(' ') ? after : ' ' + after;

	return [before, after];
}

interface InlineCodeInsertionParams {
	selection: BaseSelection | null;
	start: number;
	end: number;
	code: string;
}

export function insertInlineCode(params: InlineCodeInsertionParams) {
	const { selection, start, end, code } = params;
	if (!isRangeSelection(selection)) return;

	const nodes = selection.getNodes();

	// Has to be the last node of the selection since we are replacing the
	// last thing the user typed.
	const lastNode = nodes[nodes.length - 1];
	if (!isTextNode(lastNode)) return;

	const text = lastNode.getTextContent();
	const [before, after] = getTextSurroundingCode(text, start, end);

	lastNode.setTextContent(before);

	const inlineCode = createInlineCodeNode(code);

	lastNode.insertAfter(inlineCode);
	const suffix = inlineCode.insertAfter(createTextNode(after));
	suffix.selectEnd();
}

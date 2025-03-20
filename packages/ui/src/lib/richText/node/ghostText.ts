import {
	TextNode,
	type EditorConfig,
	type NodeKey,
	type SerializedTextNode,
	type Spread
} from 'lexical';

export type SerializedGhostTextNode = Spread<
	{
		type: 'ghostText';
		index: number | undefined;
	},
	SerializedTextNode
>;

export class GhostText extends TextNode {
	__index?: number;

	static getType(): string {
		return 'ghostText';
	}

	static clone(node: GhostText): GhostText {
		return new GhostText(node.__text, node.__key);
	}

	constructor(text: string, key?: NodeKey, index?: number) {
		super(text, key);
		this.__index = index;
	}

	createDOM(config: EditorConfig): HTMLElement {
		const dom = document.createElement('span');
		const inner = super.createDOM(config);
		dom.className = 'ghost-text';
		if (this.__index !== undefined) {
			dom.style.animationDelay = `${this.__index * 0.1}s`;
		}
		inner.className = 'ghost-text-inner';
		dom.appendChild(inner);
		return dom;
	}

	static importJSON(serializedNode: SerializedGhostTextNode): GhostText {
		const node = createGhostTextNode(serializedNode.text, serializedNode.index);
		node.setFormat(serializedNode.format);
		node.setDetail(serializedNode.detail);
		node.setMode(serializedNode.mode);
		node.setStyle(serializedNode.style);
		return node;
	}

	exportJSON(): SerializedGhostTextNode {
		return {
			...super.exportJSON(),
			type: 'ghostText',
			index: this.__index
		};
	}

	updateDOM(prevNode: TextNode, dom: HTMLElement, config: EditorConfig): boolean {
		const inner = dom.firstChild;
		if (inner === null) {
			return true;
		}
		super.updateDOM(prevNode, inner as HTMLElement, config);
		return false;
	}

	canInsertTextBefore(): boolean {
		return true;
	}

	canInsertTextAfter(): boolean {
		return false;
	}

	isTextEntity(): boolean {
		return true;
	}

	getHiddenText(): string {
		return this.__text;
	}

	getTextContent(): string {
		// GhostText should not be included in the text content
		return '';
	}
}

export function createGhostTextNode(text: string, index?: number, key?: NodeKey): GhostText {
	return new GhostText(text, key, index).setMode('token');
}

export function isGhostTextNode(node: TextNode): node is GhostText {
	return node instanceof GhostText;
}

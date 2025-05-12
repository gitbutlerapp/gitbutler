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

const MENTION_TRIGGER = '@';
const MENTION_REGEX = /(^|\s)(@([a-zA-Z0-9_-]*))$/;

export function embedUserMention(id: string, label: string): string {
	return `<<${MENTION_TRIGGER + id}:${label}>>`;
}

export function extractUserMention(text: string): [string, string] | null {
	const match = text.match(/^<<@([a-zA-Z0-9_-]+):(@[a-zA-Z0-9_-]+)>>$/);
	if (match === null) {
		return null;
	}

	const id = match[1];
	const label = match[2];
	return [id, label];
}

export type MentionMatch = {
	end: number;
	start: number;
	username: string;
};

/**
 * Return information about the mention match in the text, if there's any
 */
export function getMentionMatch(text: string): MentionMatch | null {
	const matchArr = MENTION_REGEX.exec(text);
	if (matchArr === null) {
		return null;
	}

	const mentionLength = matchArr[3].length + 1;
	const startOffset = matchArr.index + matchArr[1].length;
	const endOffset = startOffset + mentionLength;
	const username = matchArr[3];

	return {
		end: endOffset,
		start: startOffset,
		username
	};
}

export type SerializedMentionNode = Spread<
	{
		type: 'mention';
		id: string;
		label: string;
	},
	SerializedTextNode
>;

export class MentionNode extends TextNode {
	__id: string;
	__label: string;

	static getType(): string {
		return 'mention';
	}

	static clone(node: MentionNode): MentionNode {
		return new MentionNode(node.__id, node.__label, node.__key);
	}

	constructor(id: string, label: string, key?: NodeKey) {
		super(label, key);
		this.__id = id;
		this.__label = label;
	}

	createDOM(config: EditorConfig): HTMLElement {
		const dom = document.createElement('span');
		const inner = super.createDOM(config);
		dom.className = 'mention';
		inner.className = 'mention-inner';
		dom.appendChild(inner);
		return dom;
	}

	static importJSON(serializedNode: SerializedMentionNode): MentionNode {
		const node = createMentionNode(serializedNode.id, serializedNode.label);
		node.setFormat(serializedNode.format);
		node.setDetail(serializedNode.detail);
		node.setMode(serializedNode.mode);
		node.setStyle(serializedNode.style);
		return node;
	}

	exportJSON(): SerializedMentionNode {
		return {
			...super.exportJSON(),
			type: 'mention',
			id: this.__id,
			label: this.__label
		};
	}

	updateDOM(prevNode: this, dom: HTMLElement, config: EditorConfig): boolean {
		const inner = dom.firstChild;
		if (inner === null) {
			return true;
		}
		super.updateDOM(prevNode, inner as HTMLElement, config);
		return false;
	}

	canInsertTextBefore(): boolean {
		return false;
	}

	canInsertTextAfter(): boolean {
		return true;
	}

	isTextEntity(): true {
		return true;
	}

	getTextContent(): string {
		return embedUserMention(this.__id, this.__label);
	}
}

export function createMentionNode(id: string, text: string): MentionNode {
	const label = text;
	return new MentionNode(id, label).setMode('token');
}

export function isMentionNode(node: LexicalNode): node is MentionNode {
	return node instanceof MentionNode;
}

function getTextSurroundingMention(text: string, start: number, end: number): [string, string] {
	const before = text.slice(0, start);
	let after = text.slice(end);
	after = after.startsWith(' ') ? after : ' ' + after;

	return [before, after];
}

interface MentionInsertionParams {
	selection: BaseSelection | null;
	start: number;
	end: number;
	id: string;
	label: string;
}

export function insertMention(params: MentionInsertionParams) {
	const { selection, start, end, id, label } = params;
	if (!isRangeSelection(selection)) return;

	const nodes = selection.getNodes();

	// Has to be the last node of the selection since we are replacing the
	// last thing the user typed.
	const lastNode = nodes[nodes.length - 1];
	if (!isTextNode(lastNode)) return;

	const text = lastNode.getTextContent();
	const [before, after] = getTextSurroundingMention(text, start, end);

	lastNode.setTextContent(before);

	const mention = createMentionNode(id, MENTION_TRIGGER + label);

	lastNode.insertAfter(mention);
	const suffix = mention.insertAfter(createTextNode(after));
	suffix.selectEnd();
}

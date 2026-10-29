import {
	$getRoot,
	$getSelection,
	$isRangeSelection,
	$nodesOfType,
	TextNode,
	type EditorConfig,
	type LexicalEditor,
	type NodeKey,
	type SerializedTextNode,
	type Spread,
} from "lexical";

const APPEAR_DURATION_MS = 400;

type GhostTextOptions = {
	index?: number;
	total?: number;
};

export type SerializedGhostTextNode = Spread<
	{
		type: "ghostText";
		index: number | undefined;
		total: number | undefined;
	},
	SerializedTextNode
>;

export class GhostText extends TextNode {
	__index?: number;
	__total?: number;

	static getType(): string {
		return "ghostText";
	}

	static clone(node: GhostText): GhostText {
		return new GhostText(node.__text, node.__key);
	}

	constructor(text: string, key?: NodeKey, options?: GhostTextOptions) {
		super(text, key);
		this.__index = options?.index;
		this.__total = options?.total;
	}

	private getAnimationDelay(): string {
		if (this.__index !== undefined && this.__total !== undefined) {
			const fraction = APPEAR_DURATION_MS / this.__total;
			return `${this.__index * fraction}ms`;
		}
		return "";
	}

	createDOM(config: EditorConfig): HTMLElement {
		const dom = document.createElement("span");
		const inner = super.createDOM(config);
		dom.className = "ghost-text";
		dom.style.animationDelay = this.getAnimationDelay();
		inner.className = "ghost-text-inner";
		dom.appendChild(inner);
		return dom;
	}

	static importJSON(serializedNode: SerializedGhostTextNode): GhostText {
		const node = createGhostTextNode(serializedNode.text, {
			index: serializedNode.index,
			total: serializedNode.total,
		});
		node.setFormat(serializedNode.format);
		node.setDetail(serializedNode.detail);
		node.setMode(serializedNode.mode);
		node.setStyle(serializedNode.style);
		return node;
	}

	exportJSON(): SerializedGhostTextNode {
		return {
			...super.exportJSON(),
			type: "ghostText",
			index: this.__index,
			total: this.__total,
		};
	}

	/**
	 * Casting to any since the base class method signature uses `this` as the
	 * type for `prevNode`, which TypeScript interprets literally as an instance
	 * of the current class, not a subclass.
	 */
	updateDOM(prevNode: GhostText, dom: HTMLElement, config: EditorConfig): boolean {
		if (dom.firstChild === null) {
			return true;
		}
		return super.updateDOM(prevNode as any, dom, config);
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
		return "";
	}
}

export function createGhostTextNode(
	text: string,
	options?: GhostTextOptions,
	key?: NodeKey,
): GhostText {
	return new GhostText(text, key, options).setMode("token");
}

export function isGhostTextNode(node: TextNode): node is GhostText {
	return node instanceof GhostText;
}

export function insertGhostTextAtCaret(editor: LexicalEditor, ghostText: string) {
	editor.update(() => {
		const selection = $getSelection();
		if (!$isRangeSelection(selection)) {
			return;
		}

		const currentTextContent = $getRoot().getTextContent();
		let textToInsert = ghostText;
		if (ghostText.startsWith(currentTextContent)) {
			textToInsert = textToInsert.slice(currentTextContent.length);
		}

		const words = textToInsert.split(" ");

		const key = selection.focus.key;
		const offset = selection.focus.offset;
		const type = selection.focus.type;

		const total = words.length;

		const nodesToInsert = words.map((word, index) => {
			const isLast = index === words.length - 1;
			const wordWithSpace = isLast ? word : `${word} `;
			return createGhostTextNode(wordWithSpace, { index, total });
		});
		selection.insertNodes(nodesToInsert);
		selection.focus.set(key, offset, type);
		selection.anchor.set(key, offset, type);
	});
}

export function removeAllGhostText(editor: LexicalEditor) {
	editor.update(() => {
		const selection = $getSelection();
		if (!$isRangeSelection(selection)) {
			return;
		}

		const nodes = $nodesOfType(GhostText);
		for (const node of nodes) {
			node.remove();
		}
	});
}

export function replaceGhostTextWithText(editor: LexicalEditor) {
	editor.update(() => {
		const selection = $getSelection();
		if (!$isRangeSelection(selection)) {
			return;
		}

		const nodes = $nodesOfType(GhostText);
		let lastNode: TextNode | undefined;
		for (const node of nodes) {
			const text = node.getHiddenText();
			lastNode = new TextNode(text);
			node.replace(lastNode);
		}
		lastNode?.selectEnd();
	});
}

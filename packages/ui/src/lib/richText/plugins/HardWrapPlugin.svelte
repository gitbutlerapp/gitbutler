<script lang="ts" module>
	export type FormatStyle =
		| 'text-bold'
		| 'text-italic'
		| 'text-underline'
		| 'text-strikethrough'
		| 'text-code'
		| 'text-quote'
		| 'text-link'
		| 'text'
		| 'text-h1'
		| 'text-h2'
		| 'text-h3'
		| 'bullet-list'
		| 'number-list'
		| 'checklist';
</script>

<script lang="ts">
	import { wrapLine } from '$lib/richText/plugins/linewrap';

	import {
		$isTextNode as isTextNode,
		TextNode,
		$getRoot as getRoot,
		type LexicalNode,
		$isParagraphNode as isParagraphNode,
		$isLineBreakNode as isLineBreakNode,
		type NodeKey,
		type NodeMutation,
		$getNodeByKey as getNodeByKey,
		LineBreakNode,
		$createRangeSelection as createRangeSelection,
		$setSelection as setSelection,
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection
	} from 'lexical';
	import { onMount } from 'svelte';
	import { getEditor } from 'svelte-lexical';

	type Props = {
		maxLength?: number;
	};

	const { maxLength = 74 }: Props = $props();

	const editor = getEditor();
	type Bullet = { prefix: string; indent: string };

	function parseBullet(text: string): Bullet | null {
		const match = text.match(/^(\s*)([-*+]|[0-9]+\.)\s+/);
		if (!match) return null;
		const indent = match[1] ?? '';
		const prefix = match[0];
		return { prefix, indent: indent + '  ' };
	}

	let lastCheckedLine: undefined | string = undefined;
	let lastCheckedResult = false;

	/**
	 * Checks if given node id is contained within a code block.
	 */
	function isInCodeBlock(nodeId: string): boolean {
		if (lastCheckedLine === nodeId) {
			return lastCheckedResult;
		}
		const rootParagraph = getRoot().getFirstChild();
		if (!isParagraphNode(rootParagraph)) {
			throw new Error('Expected paragraph node as first child');
		}

		let node = rootParagraph.getFirstChild();
		if (node === null) return false; // Empty doc.

		let codeBlockOpen = false;

		while (node && node.getKey() !== nodeId) {
			const line = node.getTextContent();
			if (line.startsWith('```')) {
				codeBlockOpen = !codeBlockOpen;
			}
			node = node?.getNextSibling() || null;
		}

		lastCheckedLine = nodeId;
		lastCheckedResult = codeBlockOpen;
		return codeBlockOpen;
	}

	/**
	 * Returns the provided node, in addition to nodes that follow and are
	 * considered part of the same paragraph. This enables us to re-wrap
	 * a paragraph when edited in the middle.
	 */
	function getLinesToFormat(node: TextNode): TextNode[] {
		const nodes: TextNode[] = [];

		// Iterator for finding the rest of the paragraph.
		let n: LexicalNode | null = node;

		while (n) {
			if (!isTextNode(n) || n.getTextContent().trimStart() === '') {
				break;
			}
			nodes.push(n);
			n = n.getNextSibling();
			if (!n) {
				break;
			} else if (!isLineBreakNode(n)) {
				throw new Error('Expected line break node');
			}
			n = n.getNextSibling();
		}

		console.log(nodes.slice(1).length);
		return nodes.slice(1);
	}

	function maybeTransformParagraph(node: TextNode, indent: string = '') {
		const selection = getSelection();
		const text = node.getTextContent();
		if (text.length <= maxLength || text.indexOf(' ') === -1) {
			return;
		}

		/** Number of characters into the next line that the cursor should be moved. */
		let selectionCarry: number | undefined = undefined;

		// An array for collecting all modified and inserted text.
		let insertedNodes: TextNode[] = [];

		// Remainder string that should be carried over between lines when
		// re-wrapping lines.
		let remainder = '';

		// We want to consider the modified line, and the remaining lines from
		// the same pagraph.
		const relatedNodes = getLinesToFormat(node);

		const line = node.getTextContent();
		const { newLine, newRemainder } = wrapLine({ line, remainder, maxLength, indent });

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
			const selection = createRangeSelection();
			const newKey = newNode
				.insertAfter(new LineBreakNode())!
				.insertAfter(new TextNode(''))!
				.getKey();
			selection.anchor.set(newKey, 0, 'text');
			selection.focus.set(newKey, 0, 'text');
			setSelection(selection);
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
				const newNode = new TextNode(newLine);
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
				const selection = createRangeSelection();
				selection.anchor.set(secondNode.getKey(), selectionCarry, 'text');
				selection.focus.set(secondNode.getKey(), selectionCarry, 'text');
				setSelection(selection);
			}
		}
	}

	function maybeTransform(nodes: Map<NodeKey, NodeMutation>) {
		editor.update(() => {
			for (const [nodeKey, mutation] of nodes.entries()) {
				// Process additions and updates.
				if (mutation !== 'updated') {
					continue;
				}
				if (isInCodeBlock(nodeKey)) {
					continue;
				}

				const node = getNodeByKey(nodeKey);
				if (!node || !isTextNode(node)) {
					continue;
				}

				if (parseBullet(node.getTextContent())) {
					// TODO: Iplement wrapping for bullet points.
					continue;
				} else {
					maybeTransformParagraph(node);
				}
			}
		});
	}

	onMount(() => {
		const unregister = editor.registerMutationListener(TextNode, maybeTransform);
		return unregister;
	});
</script>

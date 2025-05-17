<script lang="ts">
	import {
		parseIndent,
		isWrappingExempt,
		wrapLine,
		type Bullet,
		parseBullet
	} from '$lib/richText/plugins/linewrap';
	import { mergeUnlisten } from '$lib/utils/mergeUnlisten';

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
			// Check for decrease in indent.
			if (!line.startsWith(indent)) {
				break;
			}
			// Check for increase in indent.
			if (line.length > indent.length && line[indent.length].match(/\s/)) {
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

	function maybeTransformText(node: TextNode, indent: string, bullet?: Bullet) {
		const selection = getSelection();
		const line = node.getTextContent();

		if (line.length <= maxLength || line.indexOf(' ') === -1) {
			return; // Line does not exceed max length.
		}
		if (isWrappingExempt(line)) {
			return; // Line contains text that should not be wrapped.
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

	function maybeTransform(nodes: Map<NodeKey, NodeMutation>) {
		editor.update(() => {
			for (const [nodeKey, mutation] of nodes.entries()) {
				const node = getNodeByKey(nodeKey)!;

				if (!node) {
					continue;
				}

				if (mutation === 'updated') {
					// Process updates.
					if (isInCodeBlock(nodeKey)) {
						continue;
					}

					if (!isTextNode(node)) {
						continue;
					}

					const line = node.getTextContent();

					if (line.length > maxLength) {
						const bullet = parseBullet(line);
						const indent = bullet ? bullet.indent : parseIndent(line);
						maybeTransformText(node, indent, bullet);
					}
				}
			}
		});
	}

	function moveCursorTo(nodeKey: string, position: number) {
		const selection = createRangeSelection();
		selection.anchor.set(nodeKey, position, 'text');
		selection.focus.set(nodeKey, position, 'text');
		setSelection(selection);
	}

	onMount(() => {
		return mergeUnlisten(editor.registerMutationListener(TextNode, maybeTransform));
	});
</script>

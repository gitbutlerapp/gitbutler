<script lang="ts">
	import { getEditor } from '$lib/richText/context';
	import { createInlineCodeNode, isInlineCodeNode } from '$lib/richText/node/inlineCode';
	import {
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection,
		$isTextNode as isTextNode,
		$createTextNode as createTextNode,
		type TextNode
	} from 'lexical';

	const editor = getEditor();

	/**
	 * Parse a text node's content and split it into text and inline code nodes
	 */
	function parseAndReplaceTextNode(node: TextNode) {
		const text = node.getTextContent();
		const backtickRegex = /`([^`]+)`/g;
		const matches: Array<{ start: number; end: number; code: string }> = [];
		let match;

		// Find all backtick matches
		while ((match = backtickRegex.exec(text)) !== null) {
			matches.push({
				start: match.index,
				end: backtickRegex.lastIndex,
				code: match[1]
			});
		}

		// If no matches, nothing to do
		if (matches.length === 0) {
			return;
		}

		// Build array of nodes to replace the current text node
		const newNodes: Array<TextNode> = [];
		let lastIndex = 0;

		for (const match of matches) {
			// Add text before the match
			if (match.start > lastIndex) {
				const beforeText = text.slice(lastIndex, match.start);
				newNodes.push(createTextNode(beforeText));
			}

			// Add inline code node
			newNodes.push(createInlineCodeNode(match.code));

			lastIndex = match.end;
		}

		// Add remaining text after the last match
		if (lastIndex < text.length) {
			const remainingText = text.slice(lastIndex);
			newNodes.push(createTextNode(remainingText));
		}

		// Replace the node with the new nodes
		if (newNodes.length > 0) {
			const firstNode = newNodes[0];
			node.replace(firstNode);

			// Insert the rest of the nodes after the first one
			let prevNode = firstNode;
			for (let i = 1; i < newNodes.length; i++) {
				prevNode.insertAfter(newNodes[i]);
				prevNode = newNodes[i];
			}

			// Restore selection - always select the end of the last node
			const lastNode = newNodes[newNodes.length - 1];
			lastNode.selectEnd();
		}
	}

	$effect(() => {
		return editor.registerUpdateListener(({ editorState, dirtyLeaves, tags }) => {
			if (tags.has('history-merge') || dirtyLeaves.size === 0) {
				return;
			}

			editorState.read(() => {
				const selection = getSelection();
				if (!isRangeSelection(selection)) {
					return;
				}

				const anchorNode = selection.anchor.getNode();
				if (!isTextNode(anchorNode) || isInlineCodeNode(anchorNode)) {
					return;
				}

				// Only process if the text contains backticks
				const text = anchorNode.getTextContent();
				if (!text.includes('`')) {
					return;
				}

				// Check if we have complete backtick pairs
				const backtickCount = (text.match(/`/g) || []).length;
				if (backtickCount < 2 || backtickCount % 2 !== 0) {
					// Incomplete pairs, don't process yet
					return;
				}

				// Parse and replace the text node
				editor.update(() => {
					parseAndReplaceTextNode(anchorNode);
				});
			});
		});
	});
</script>

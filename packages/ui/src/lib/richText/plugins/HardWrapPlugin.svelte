<!--
@component
Lexical plugin that wraps a line exceeding given max length.

The implementation of the wrapping is handled by a lib file since it needs
to be shared with e.g. wrapping rich text converted into markdown.

A known shortcoming is when you have a pasted paragraph in the editor, which
gets wrapped when edited, so if your cursor is on a  line that gets wrapped
a few rows down we do not update the cursor position to correctly.
-->
<script lang="ts">
	import { WRAP_ALL_COMMAND } from '$lib/richText/commands';
	import { wrapAll, wrapIfNecessary } from '$lib/richText/linewrap';
	import {
		$isTextNode as isTextNode,
		TextNode,
		type NodeKey,
		type NodeMutation,
		$getNodeByKey as getNodeByKey,
		COMMAND_PRIORITY_NORMAL
	} from 'lexical';
	import { getEditor } from 'svelte-lexical';

	type Props = {
		maxLength: number | undefined;
		enabled?: boolean;
	};

	const { maxLength, enabled }: Props = $props();

	const editor = getEditor();

	let lastCheckedLine: undefined | string = undefined;
	let lastCheckedResult = false;
	let hasInitialized = false;

	// Auto-wrap content on mount if enabled
	$effect(() => {
		if (enabled && maxLength && !hasInitialized) {
			hasInitialized = true;
			setTimeout(() => wrapAll(editor, maxLength), 0);
		}
	});

	$effect(() => {
		if (enabled) {
			return editor.registerMutationListener(TextNode, onTextNodeMutation);
		}
	});

	$effect(() =>
		editor.registerCommand(
			WRAP_ALL_COMMAND,
			() => {
				if (enabled && maxLength) {
					wrapAll(editor, maxLength);
				}
				return true;
			},
			COMMAND_PRIORITY_NORMAL
		)
	);

	function onTextNodeMutation(nodes: Map<NodeKey, NodeMutation>) {
		editor.update(
			() => {
				for (const [key, type] of nodes.entries()) {
					const node = getNodeByKey(key)!;

					if (!node) {
						continue;
					}

					if (type === 'updated' && maxLength) {
						const inCodeBlock = isInCodeBlock(key);
						if (inCodeBlock) {
							continue;
						}
						if (!isTextNode(node)) {
							continue;
						}
						wrapIfNecessary({ node, maxLength });
					}
				}
			},
			{
				// Merge with the current history entry so wrapping doesn't create separate undo steps
				tag: 'history-merge'
			}
		);
	}

	/**
	 * Checks if given node id is contained within a code block.
	 * In the new multi-paragraph structure, code blocks are CodeNodes within paragraphs.
	 */
	function isInCodeBlock(nodeId: string): boolean {
		if (lastCheckedLine === nodeId) {
			return lastCheckedResult;
		}

		const node = getNodeByKey(nodeId);
		if (!node) {
			lastCheckedLine = nodeId;
			lastCheckedResult = false;
			return false;
		}

		// Check if this node or any parent is a CodeNode
		let current: any = node;
		let depth = 0;
		while (current) {
			if (current.getType && current.getType() === 'code') {
				lastCheckedLine = nodeId;
				lastCheckedResult = true;
				return true;
			}
			current = current.getParent ? current.getParent() : null;
			depth++;

			if (depth > 10) {
				console.warn(
					'[HardWrap] Parent chain depth exceeded 10, stopping to prevent infinite loop'
				);
				break;
			}
		}

		lastCheckedLine = nodeId;
		lastCheckedResult = false;
		return false;
	}
</script>

<!--
@component
Lexical plugin that wraps a line exceeding given max length.

The implementation of the wrapping is handled by a lib file since it needs
to be shared with e.g. wrapping rich text converted into markdown.
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
					if (type !== 'updated' || !maxLength) continue;

					const node = getNodeByKey(key);
					if (!node || !isTextNode(node) || isInCodeBlock(node)) continue;

					wrapIfNecessary({ node, maxLength });
				}
			},
			{
				// Merge with the current history entry so wrapping doesn't create separate undo steps
				tag: 'history-merge'
			}
		);
	}

	/**
	 * Checks if given node is contained within a code block.
	 * In the new multi-paragraph structure, code blocks are CodeNodes within paragraphs.
	 */
	function isInCodeBlock(node: any): boolean {
		let current = node;
		let depth = 0;

		while (current) {
			if (current.getType?.() === 'code') {
				return true;
			}
			current = current.getParent?.();
			depth++;

			if (depth > 10) {
				console.warn(
					'[HardWrap] Parent chain depth exceeded 10, stopping to prevent infinite loop'
				);
				return false;
			}
		}

		return false;
	}
</script>

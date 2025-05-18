<!--
@component
Lexical plugin that wraps a line exceeding given max length.

The implementation of the wrapping is handled by a lib file since it needs
to be shared with e.g. wrapping rich text converted into markdown.

A known shortcoming is when you have a pasted paragraph in the editor, which
gets wrapped when edited, so if your cursor is on a  line that gets wrapped
a few rows down we do not update the cursor position to correctly.

TODO: Validate the editor is in plain text mode when this plugin is active.
-->
<script lang="ts">
	import { WRAP_EXEMPTIONS, wrapIfNecssary } from '$lib/richText/linewrap';
	import { mergeUnlisten } from '$lib/utils/mergeUnlisten';
	import {
		$isTextNode as isTextNode,
		TextNode,
		$getRoot as getRoot,
		$isParagraphNode as isParagraphNode,
		type NodeKey,
		type NodeMutation,
		$getNodeByKey as getNodeByKey
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

	onMount(() => {
		return mergeUnlisten(editor.registerMutationListener(TextNode, onTextNodeMutation));
	});

	function onTextNodeMutation(nodes: Map<NodeKey, NodeMutation>) {
		editor.update(() => {
			for (const [key, type] of nodes.entries()) {
				const node = getNodeByKey(key)!;

				if (!node) {
					continue;
				}

				if (type === 'updated') {
					if (isInCodeBlock(key)) {
						continue;
					}
					if (!isTextNode(node)) {
						continue;
					}
					wrapIfNecssary({ node, maxLength });
				}
			}
		});
	}

	/**
	 * Checks if given node id is contained within a code block.
	 *
	 * You don't want to call this from within a loop, since then
	 * you will end up with an O(n^2) loop.
	 *
	 * It is a known limitation that it has no state machine, so
	 * open/close matches could be different, e.g. ~~~ vs. ```.
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
			if (WRAP_EXEMPTIONS.FencedCodeBlock.test(line)) {
				codeBlockOpen = !codeBlockOpen;
			}
			node = node?.getNextSibling() || null;
		}

		lastCheckedLine = nodeId;
		lastCheckedResult = codeBlockOpen;
		return codeBlockOpen;
	}
</script>

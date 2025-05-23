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
	import { WRAP_EXEMPTIONS, wrapAll, wrapIfNecssary } from '$lib/richText/linewrap';
	import {
		$isTextNode as isTextNode,
		TextNode,
		$getRoot as getRoot,
		$isParagraphNode as isParagraphNode,
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

	$effect(() => {
		if (enabled) {
			return editor.registerMutationListener(TextNode, onTextNodeMutation);
		}
	});

	$effect(() => {
		editor.registerCommand(
			WRAP_ALL_COMMAND,
			() => {
				if (enabled && maxLength) {
					wrapAll(editor, maxLength);
				}
				return true;
			},
			COMMAND_PRIORITY_NORMAL
		);
	});

	function onTextNodeMutation(nodes: Map<NodeKey, NodeMutation>) {
		editor.update(
			() => {
				for (const [key, type] of nodes.entries()) {
					const node = getNodeByKey(key)!;

					if (!node) {
						continue;
					}

					if (type === 'updated' && maxLength) {
						if (isInCodeBlock(key)) {
							continue;
						}
						if (!isTextNode(node)) {
							continue;
						}
						wrapIfNecssary({ node, maxLength });
					}
				}
			},
			{
				// Allows undoing the wrap.
				tag: 'history'
			}
		);
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

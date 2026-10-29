<script lang="ts">
	import { getEditor } from "$lib/richText/context";
	import { createInlineCodeNode, isInlineCodeNode } from "$lib/richText/node/inlineCode";
	import { TextNode } from "lexical";

	const INLINE_CODE_REGEX = /`[^`]+`/;

	/**
	 * Node transform that converts backtick-wrapped text in TextNodes into
	 * InlineCodeNodes. Unlike the old text-match transformer approach, this
	 * fires on any TextNode mutation — so it handles cases like restoring a
	 * deleted opening backtick, paste, and programmatic changes.
	 *
	 * After splitting, any remaining sibling TextNodes will be marked dirty
	 * by Lexical and will receive their own transform call, so we only need
	 * to handle one match per invocation.
	 */
	function inlineCodeTextNodeTransform(node: TextNode): void {
		if (isInlineCodeNode(node)) {
			return;
		}

		if (!node.isSimpleText()) {
			return;
		}

		const text = node.getTextContent();
		const match = INLINE_CODE_REGEX.exec(text);

		if (!match || match.index === undefined) {
			return;
		}

		const startIndex = match.index;
		const endIndex = startIndex + match[0].length;

		const codeNode = createInlineCodeNode(match[0]);

		let replaceNode: TextNode;
		if (startIndex === 0) {
			[replaceNode] = node.splitText(endIndex);
		} else {
			[, replaceNode] = node.splitText(startIndex, endIndex);
		}

		replaceNode.replace(codeNode);
	}

	const editor = getEditor();

	$effect(() => {
		const unregister = editor.registerNodeTransform(TextNode, inlineCodeTextNodeTransform);
		return () => {
			unregister?.();
		};
	});
</script>

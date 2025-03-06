<script lang="ts">
	import { getEditor } from '../context';
	import { $createEmojiNode as createEmojiNode } from '../node/emoji';
	import { TextNode } from 'lexical';

	const emojis: Map<string, [string, string]> = new Map([
		[':)', ['emoji happysmile', '🙂']],
		[':D', ['emoji veryhappysmile', '😀']],
		[':(', ['emoji unhappysmile', '🙁']],
		['<3', ['emoji heart', '❤']]
	]);

	function findAndTransformEmoji(node: TextNode): null | TextNode {
		const text = node.getTextContent();

		for (let i = 0; i < text.length; i++) {
			const emojiData = emojis.get(text[i]!) || emojis.get(text.slice(i, i + 2));

			if (emojiData !== undefined) {
				const [emojiStyle, emojiText] = emojiData;
				let targetNode;

				if (i === 0) {
					[targetNode] = node.splitText(i + 2);
				} else {
					[, targetNode] = node.splitText(i, i + 2);
				}

				const emojiNode = createEmojiNode(emojiStyle, emojiText);
				targetNode!.replace(emojiNode);
				return emojiNode;
			}
		}

		return null;
	}

	function emojiTextNodeTransform(node: TextNode): void {
		let targetNode: TextNode | null = node;

		while (targetNode !== null) {
			if (!targetNode.isSimpleText()) {
				return;
			}

			targetNode = findAndTransformEmoji(targetNode);
		}
	}

	const editor = getEditor();

	$effect(() => {
		const unregister = editor.registerNodeTransform(TextNode, emojiTextNodeTransform);
		return () => {
			unregister?.();
		};
	});
</script>

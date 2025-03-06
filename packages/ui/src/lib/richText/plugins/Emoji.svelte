<script lang="ts">
	import { getEditor } from '../context';
	import { $createEmojiNode as createEmojiNode } from '../node/emoji';
	import emojiData from 'emojibase-data/en/compact.json';
	import emojiByHexcode from 'emojibase-data/en/shortcodes/github.json';
	import { TextNode } from 'lexical';
	import type { CompactEmoji } from 'emojibase';

	const emojis: Map<string, [string, string]> = new Map([
		[':)', ['emoji happysmile', '🙂']],
		[':D', ['emoji veryhappysmile', '😀']],
		[':(', ['emoji unhappysmile', '🙁']],
		['<3', ['emoji heart', '\u{2764}']],
		[':P', ['emoji tongue', '😛']],
		[':O', ['emoji surprised', '😲']],
		[':|', ['emoji neutral', '😐']],
		[':/', ['emoji unsure', '😕']],
		[':S', ['emoji confused', '😕']],
		[':X', ['emoji lipssealed', '🤐']],
		[':*', ['emoji kiss', '😘']],
		[':o', ['emoji shocked', '😮']],
		[':s', ['emoji smile', '😊']],
		[':x', ['emoji angry', '😠']],
		[':p', ['emoji cheeky', '😜']],
		[':]', ['emoji evil', '😈']],
		[':(', ['emoji sad', '😞']],
		[':)', ['emoji happy', '😊']],
		[':D', ['emoji bigsmile', '😃']],
		[':o', ['emoji wow', '😮']],
		[':O', ['emoji wow', '😮']],
		[':P', ['emoji cheeky', '😜']],
		[':p', ['emoji cheeky', '😜']],
		[':s', ['emoji smile', '😊']],
		[':S', ['emoji smile', '😊']],
		[':x', ['emoji angry', '😠']],
		[':X', ['emoji angry', '😠']],
		[':|', ['emoji neutral', '😐']],
		[':/', ['emoji unsure', '😕']],
		[':]', ['emoji evil', '😈']],
		[':(', ['emoji sad', '😞']]
	]);

	const EMOJI_SHORTCODE_REGEX = /(^|\s):([0-9a-z+_-]+):$/g;

	function findEmojiByShortcode(shortcode: string): CompactEmoji | undefined {
		const emoji = Object.entries(emojiByHexcode).find(([_, shortCodes]) => {
			if (Array.isArray(shortCodes)) {
				return shortCodes.includes(shortcode);
			}
			return shortCodes === shortcode;
		});

		if (!emoji) {
			return undefined;
		}

		const compactEmoji = emojiData.find((e) => e.hexcode === emoji[0]);
		return compactEmoji;
	}

	type EmojiMatch = {
		start: number;
		end: number;
		shortCode: string;
	};

	function getShortCodeMonitorMatch(text: string): EmojiMatch | null {
		const testResult = EMOJI_SHORTCODE_REGEX.exec(text);

		if (!testResult) {
			return null;
		}

		const shortCode = testResult[2];
		const start = testResult.index + testResult[1].length;
		const end = start + shortCode.length + 2; // Account for the colons

		return { start, end, shortCode };
	}

	function getNodeToReplace(node: TextNode, index: number, length: number): TextNode {
		if (index === 0) {
			const [targetNode] = node.splitText(length);
			return targetNode;
		}

		const [, targetNode] = node.splitText(index, index + length);
		return targetNode;
	}

	function findShortCodeEmoji(node: TextNode): TextNode | undefined {
		const text = node.getTextContent();

		const shortCodeMatch = getShortCodeMonitorMatch(text);
		if (!shortCodeMatch) {
			return undefined;
		}

		const match = findEmojiByShortcode(shortCodeMatch.shortCode);

		if (!match) {
			return undefined;
		}

		const emojiNode = createEmojiNode('emoji', match.unicode);

		const targetNode = getNodeToReplace(node, shortCodeMatch.start, shortCodeMatch.end);
		targetNode.replace(emojiNode);
		return emojiNode;
	}

	function findAndTransformEmoji(node: TextNode): TextNode | undefined {
		const text = node.getTextContent();

		for (let i = 0; i < text.length; i++) {
			const emojiData = emojis.get(text[i]!) || emojis.get(text.slice(i, i + 2));

			if (emojiData !== undefined) {
				const [emojiStyle, emojiText] = emojiData;
				const targetNode = getNodeToReplace(node, i, 2);

				const emojiNode = createEmojiNode(emojiStyle, emojiText);
				targetNode.replace(emojiNode);
				return emojiNode;
			}
		}

		return undefined;
	}

	function emojiTextNodeTransform(node: TextNode): void {
		let targetNode: TextNode | undefined = node;

		while (targetNode !== undefined) {
			if (!targetNode.isSimpleText()) {
				return;
			}

			let newTargetNode = findAndTransformEmoji(targetNode);
			newTargetNode ??= findShortCodeEmoji(targetNode);

			targetNode = newTargetNode;
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

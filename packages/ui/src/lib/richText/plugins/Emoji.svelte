<script lang="ts">
	import EmojiSuggestions, { type EmojiSuggestion } from './EmojiSuggestions.svelte';
	import TypeAheadPlugin from './TypeAhead.svelte';
	import { getEditor } from '../context';
	import {
		findAndReplaceShortCodeEmoji,
		searchThroughEmojis,
		getShortCodeSearchMatch,
		insertEmoji,
		type ShortCodeSearchMatch
	} from '../node/emoji';
	import { TextNode, $getSelection as getSelection } from 'lexical';
	import type { CompactEmoji } from 'emojibase';

	/**
	 * Transforms a text node to replace emoji shortcodes with emoji nodes.
	 */
	function emojiTextNodeTransform(node: TextNode): void {
		let targetNode: TextNode | undefined = node;

		while (targetNode !== undefined) {
			if (!targetNode.isSimpleText()) {
				return;
			}

			targetNode = findAndReplaceShortCodeEmoji(targetNode);
		}
	}

	const editor = getEditor();

	$effect(() => {
		const unregister = editor.registerNodeTransform(TextNode, emojiTextNodeTransform);
		return () => {
			unregister?.();
		};
	});

	let suggestedEmojis = $state<CompactEmoji[]>();
	let currentShortCodeMatch = $state<ShortCodeSearchMatch>();

	function onExit() {
		suggestedEmojis = undefined;
		currentShortCodeMatch = undefined;
	}

	function onMatch(shortCodeMatch: ShortCodeSearchMatch) {
		currentShortCodeMatch = shortCodeMatch;
		const emojis = searchThroughEmojis(currentShortCodeMatch.searchQuery);
		if (emojis.length === 0) {
			onExit();
			return;
		}

		suggestedEmojis = emojis.slice(0, 20);
	}

	function onSelectEmojiSuggestion(emoji: EmojiSuggestion) {
		if (currentShortCodeMatch) {
			const start = currentShortCodeMatch.start;
			const end = currentShortCodeMatch.end;
			// Replace the search text with the selected emoji
			editor.update(() => {
				const selection = getSelection();
				insertEmoji({
					selection,
					start,
					end,
					unicode: emoji.unicode
				});
			});
		}
		onExit();
	}

	/**
	 * Returns whether the emoji plugin is currently busy fetching suggestions.
	 */
	export function isBusy(): boolean {
		return suggestedEmojis !== undefined;
	}
</script>

<EmojiSuggestions {suggestedEmojis} selectSuggestion={onSelectEmojiSuggestion} exit={onExit} />
<TypeAheadPlugin {onExit} {onMatch} testMatch={getShortCodeSearchMatch} />

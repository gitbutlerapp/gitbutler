<script lang="ts">
	import Mention, { type MentionMatch } from './Mention.svelte';

	interface Props {
		text: string;
	}

	const { text }: Props = $props();

	function getMentionId(word: string): MentionMatch | undefined {
		if (!word) {
			return undefined;
		}

		const match = word.match(/(.*)<<@(\d+)>>(.*)/);
		if (match) {
			const id = parseInt(match[2]);
			const prefix = getMentionId(match[1]) ?? match[1];
			const suffix = getMentionId(match[3]) ?? match[3];

			return {
				id,
				prefix,
				suffix
			};
		}
		return undefined;
	}

	interface TextWord {
		type: 'text';
		value: string;
	}

	interface MentionWord {
		type: 'mention';
		mention: MentionMatch;
	}

	type Word = TextWord | MentionWord;

	function getWords(text: string): Word[] {
		const words: Word[] = [];
		const listedText = text.split(' ');
		for (const word of listedText) {
			const mention = getMentionId(word);

			if (mention) {
				words.push({ type: 'mention', mention });
				continue;
			}

			words.push({ type: 'text', value: word });
		}
		return words;
	}

	const words = $derived(getWords(text));
</script>

<span>
	{#each words as word}
		{#if word.type === 'text'}
			{word.value + ' '}
		{:else}
			<Mention mention={word.mention} />
			{' '}
		{/if}
	{/each}
</span>

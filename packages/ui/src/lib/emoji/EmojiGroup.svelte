<script lang="ts">
	import EmojiButton from '$lib/emoji/EmojiButton.svelte';
	import DelayedMount from '$lib/lazyness/DelayedMount.svelte';
	import type { EmojiGroup, EmojiInfo } from '$lib/emoji/utils';

	type Props = {
		index: number;
		scrollTop: number;
		group: EmojiGroup;
		handleEmojiClick: (emoji: EmojiInfo) => void;
	};

	let { index, group, handleEmojiClick }: Props = $props();
	let groupContainer = $state<HTMLDivElement>();

	const delay = $derived(index > 1 ? 500 : 0);
</script>

<div bind:this={groupContainer} class="emoji-picker__group" id="emoji-group-{group.key}">
	<DelayedMount {delay}>
		{#each group.emojis as emoji, index (emoji.unicode)}
			{@const sectionIndex = Math.floor(index / 30)}
			{@const sectionDelay = sectionIndex * 300}
			<DelayedMount delay={sectionDelay}>
				<EmojiButton emoji={emoji.unicode} onclick={() => handleEmojiClick(emoji)} />
			</DelayedMount>
		{/each}
	</DelayedMount>
</div>

<style lang="postcss">
	.emoji-picker__group {
		display: grid;
		align-items: center;
		justify-items: center;
		grid-template-columns: repeat(7, 1fr);
		grid-auto-rows: 1fr;
		padding: 8px 6px;
		row-gap: 4px;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-3);
		}
	}
</style>

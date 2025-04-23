<script lang="ts">
	import Textbox from '$lib/Textbox.svelte';
	import EmojiButton from '$lib/emoji/EmojiButton.svelte';
	import EmojiGroup from '$lib/emoji/EmojiGroup.svelte';
	import {
		getEmojiGroups,
		markRecentlyUsedEmoji,
		searchThroughEmojis,
		type EmojiGroupKey,
		type EmojiInfo
	} from '$lib/emoji/utils';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';

	type Props = {
		onEmojiSelect: (emoji: EmojiInfo) => void;
	};

	let { onEmojiSelect }: Props = $props();

	const groups = getEmojiGroups();
	let selectedGroup = $state<EmojiGroupKey>('recently-used');
	let searchVal = $state<string>();

	const searchResults = $derived.by(() => {
		if (!searchVal) return undefined;
		return searchThroughEmojis(searchVal);
	});

	function scrollIntoGroupView(selectedGroup: EmojiGroupKey) {
		const element = document.getElementById(`emoji-group-${selectedGroup}`);
		if (element) {
			element.scrollIntoView({ behavior: 'smooth', block: 'start' });
		}
	}

	function handleSelectGroup(group: EmojiGroupKey) {
		selectedGroup = group;
		scrollIntoGroupView(group);
	}

	function handleEmojiClick(emoji: EmojiInfo) {
		markRecentlyUsedEmoji(emoji);
		onEmojiSelect(emoji);
	}
</script>

<div class="emoji-picker">
	<div class="emoji-picker__header">
		<Textbox
			placeholder="Search emojis..."
			reversedDirection
			icon="search"
			bind:value={searchVal}
		/>

		<div class="emoji-picker__categories">
			{#each groups as group}
				{#if group.emojis.length !== 0}
					<button
						type="button"
						class="emoji-picker__category"
						class:selected={selectedGroup === group.key}
						onclick={() => handleSelectGroup(group.key)}
					>
						{group.unicode}
					</button>
				{/if}
			{/each}
		</div>
	</div>

	<div class="emoji-picker__body-wrapper">
		<ScrollableContainer whenToShow="scroll">
			<div class="emoji-picker__body">
				{#if searchVal && searchResults}
					{#if searchResults.length === 0}
						<div class="emoji-picker__placeholder">
							<span class="text-13">No emojis found ¯\_(ツ)_/¯ </span>
						</div>
					{:else}
						<div class="emoji-picker__group">
							{#each searchResults as emoji}
								<EmojiButton emoji={emoji.unicode} onclick={() => handleEmojiClick(emoji)} />
							{/each}
						</div>
					{/if}
				{:else}
					{#each groups as group}
						{#if group.emojis.length !== 0}
							<EmojiGroup {group} {handleEmojiClick} />
						{/if}
					{/each}
				{/if}
			</div>
		</ScrollableContainer>
	</div>
</div>

<style lang="postcss">
	.emoji-picker {
		display: flex;
		flex-direction: column;
		width: 300px;
		height: 306px;
		min-height: 0;
		background: var(--clr-bg-1);
	}

	.emoji-picker__header {
		flex: 0 0 auto;
		position: sticky;
		top: 0;
		padding: 12px;
		padding-bottom: 0;
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--clr-border-2);
		overflow: hidden;
	}

	/* CATEGORIES */
	.emoji-picker__categories {
		display: flex;
		justify-content: space-between;
		margin-top: 10px;
		margin-bottom: 8px;
	}

	.emoji-picker__category {
		position: relative;
		flex-shrink: 0;
		display: flex;
		justify-content: center;
		align-items: center;
		font-size: 14px;
		line-height: 1;
		width: 24px;
		height: 24px;
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}

		&:after {
			content: '';
			position: absolute;
			bottom: -8px;
			left: 0;
			height: 4px;
			width: 100%;
			border-radius: 4px 4px 0 0;
			background-color: var(--clr-theme-pop-element);
			transform: translateY(100%);
			transition: transform var(--transition-medium);
		}

		&.selected {
			background-color: var(--clr-bg-1-muted);

			&:after {
				transform: translateY(0);
			}
		}
	}

	.emoji-picker__body-wrapper {
		flex-grow: 1;
		min-height: 0;
		overflow: hidden;
	}

	.emoji-picker__body {
		flex-direction: column;
		display: flex;
		min-height: 100%;
	}

	.emoji-picker__group {
		display: flex;
		flex-wrap: wrap;
		padding: 8px 12px;
	}

	.emoji-picker__placeholder {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		width: 100%;
		color: var(--clr-text-3);
	}
</style>

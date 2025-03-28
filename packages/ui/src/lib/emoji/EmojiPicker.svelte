<script lang="ts">
	import Button from '$lib/Button.svelte';
	import Icon from '$lib/Icon.svelte';
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
	let search = $state<string>();

	const searchResults = $derived.by(() => {
		if (!search) return undefined;
		return searchThroughEmojis(search);
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
	<div class="emoji-picker__header-row">
		<div class="emoji-picker__search">
			<input
				type="text"
				class="text-input text-13 search-input"
				bind:value={search}
				placeholder="Search emojis..."
			/>
			<div class="emoji-picker__search-icon">
				<Icon name="search" />
			</div>
		</div>
		<div class="emoji-picker__categories-wrapper">
			<ScrollableContainer horz wide whenToShow="hover">
				<div class="emoji-picker__categories">
					{#each groups as group}
						<div class="emoji-picker__category" class:selected={selectedGroup === group.key}>
							<Button kind="ghost" onclick={() => handleSelectGroup(group.key)}>
								<p class="text-16">
									{group.unicode}
								</p>
							</Button>
						</div>
					{/each}
				</div>
			</ScrollableContainer>
		</div>
	</div>

	<div class="emoji-picker__body-wrapper">
		<ScrollableContainer whenToShow="scroll">
			<div class="emoji-picker__body">
				{#if searchResults}
					<div class="emoji-picker__group">
						{#each searchResults as emoji}
							<div class="emoji">
								<Button kind="ghost" onclick={() => handleEmojiClick(emoji)}>
									<p class="text-16">{emoji.unicode}</p>
								</Button>
							</div>
						{/each}
					</div>
				{:else}
					{#each groups as group}
						<div class="emoji-picker__group" id="emoji-group-{group.key}">
							{#each group.emojis as emoji}
								<div class="emoji">
									<Button kind="ghost" onclick={() => handleEmojiClick(emoji)}>
										<p class="text-16">{emoji.unicode}</p>
									</Button>
								</div>
							{/each}
						</div>
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
		width: 272px;
		height: 288px;
		margin: 1px;
		min-height: 0;

		background: var(--clr-bg-1);
	}

	.emoji-picker__header-row {
		padding: 12px;
		padding-bottom: 0;
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--clr-border-2);
		gap: 9px;
	}

	.emoji-picker__search {
		display: flex;
		height: var(--size-cta);
		padding: 4px 8px 4px 10px;
		align-items: center;
		gap: 8px;

		border-radius: var(--radius-s);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
	}

	.search-input {
		flex-grow: 1;
		padding-left: 10px;
		border: none;
		background: none;
		color: var(--clr-text-1);
	}

	.emoji-picker__search-icon {
		color: var(--clr-scale-ntrl-50);
	}

	.emoji-picker__categories-wrapper {
		display: flex;
	}

	.emoji-picker__categories {
		flex-grow: 1;
		display: flex;
		gap: 4px;
	}

	.emoji-picker__category {
		flex-shrink: 0;
		display: flex;
		flex-direction: column;
		justify-content: flex-start;
		align-items: center;
		width: 24px;
		height: 28px;
		box-sizing: border-box;

		&.selected {
			border-bottom: 4px solid var(--clr-theme-pop-element);
		}
	}

	.emoji-picker__body-wrapper {
		flex-grow: 1;
		min-height: 0;
	}

	.emoji-picker__body {
		flex-direction: column;
		display: flex;
	}

	.emoji-picker__group {
		display: flex;
		flex-wrap: wrap;
		padding: 8px 14px;
		gap: 3px;
		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-3);
		}
	}

	.emoji {
		display: flex;
		justify-content: center;
		align-items: center;
		width: 24px;
		height: 24px;
		box-sizing: border-box;
	}
</style>

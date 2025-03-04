<script lang="ts" module>
	import type { MentionNodeAttrs } from '@gitbutler/ui/old_RichTextEditor.svelte';

	export interface Props {
		isLoading: boolean;
		suggestions: MentionNodeAttrs[] | undefined;
		selectSuggestion?: (suggestion: MentionNodeAttrs) => void;
	}
</script>

<script lang="ts">
	import MentionSuggestionItem from './MentionSuggestionItem.svelte';
	import { setPosition } from '@gitbutler/ui/utils/tooltipPosition';
	import { flyScale } from '@gitbutler/ui/utils/transitions';

	const { suggestions, selectSuggestion, isLoading }: Props = $props();

	let selectedSuggestionIndex = $state<number>();
	let targetEl: HTMLElement | undefined = $state();

	$effect(() => {
		if (suggestions === undefined || suggestions.length === 0) {
			selectedSuggestionIndex = undefined;
		} else {
			selectedSuggestionIndex = 0;
		}
	});

	function scrollSuggestionIntoView(index: number) {
		const suggestionItem = document.getElementById(`suggestion-item-${index}`);
		if (suggestionItem) {
			suggestionItem.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
		}
	}

	export function onArrowUp() {
		if (suggestions === undefined) return;

		if (selectedSuggestionIndex === undefined) {
			selectedSuggestionIndex = suggestions.length - 1;
			return;
		}

		selectedSuggestionIndex =
			(selectedSuggestionIndex - 1 + suggestions.length) % suggestions.length;

		scrollSuggestionIntoView(selectedSuggestionIndex);
	}

	export function onArrowDown() {
		if (suggestions === undefined) return;

		if (selectedSuggestionIndex === undefined) {
			selectedSuggestionIndex = 0;
			return;
		}

		selectedSuggestionIndex = (selectedSuggestionIndex + 1) % suggestions.length;

		scrollSuggestionIntoView(selectedSuggestionIndex);
	}

	export function onEnter() {
		if (suggestions === undefined || selectedSuggestionIndex === undefined) return;

		selectSuggestion?.(suggestions[selectedSuggestionIndex]);
	}
</script>

<div bind:this={targetEl} class="suggestions-wrapper">
	<!-- Empty div needed for the position calculation -->
	<div></div>

	{#if suggestions}
		<div
			use:setPosition={{ targetEl, position: 'top', align: 'center', gap: 2 }}
			transition:flyScale
			role="presentation"
			class="popup-positioner"
		>
			<div class="suggestions">
				<ul class="suggestions-list">
					{#if suggestions.length === 0}
						<li>
							<div class="suggestion-item">
								<p class="suggestion-item__no-match text-13 text-tertiary name truncate">
									No matches found ¯\_(ツ)_/¯
								</p>
							</div>
						</li>
					{:else if isLoading}
						<li>
							<div class="suggestion-item">
								<p class="suggestion-item__no-match text-13 text-tertiary name truncate">
									Loading...
								</p>
							</div>
						</li>
					{:else}
						{#each suggestions as suggestion, idx (suggestion.id)}
							<li>
								<div
									id="suggestion-item-{idx}"
									class="suggestion-item"
									class:selected={idx === selectedSuggestionIndex}
								>
									<button type="button" onclick={() => selectSuggestion?.(suggestion)}>
										<MentionSuggestionItem username={suggestion.label} />
									</button>
								</div>
							</li>
						{/each}
					{/if}
				</ul>
			</div>
		</div>
	{/if}
</div>

<style lang="postcss">
	.suggestions-wrapper {
		transform: translateY(-10%);
		margin-bottom: 4px;
	}

	.popup-positioner {
		height: 0;
	}

	.suggestions {
		position: absolute;
		bottom: 0;
		left: 0;
		width: 100%;
	}

	.suggestions-list {
		list-style: none;
		margin: 0;

		display: flex;
		padding: 8px 7px;
		flex-direction: column;
		gap: 2px;

		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);

		box-shadow: 0px 4px 14px 0px rgba(0, 0, 0, 0.06);

		max-height: 100px;
		overflow: scroll;

		&::-webkit-scrollbar {
			display: none;
		}
	}

	.suggestion-item {
		display: flex;
		align-items: center;
		padding: 6px 8px 6px 6px;

		gap: 10px;
		height: 32px;
		overflow: hidden;
		text-align: left;
		user-select: none;
		outline: none;
		cursor: pointer;

		button {
			width: 100%;
		}

		&.selected {
			border-radius: var(--radius-m);
			background: var(--clr-bg-1-muted);
		}
	}

	.suggestion-item__no-match {
		opacity: 0.4;
	}
</style>

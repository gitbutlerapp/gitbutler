<script lang="ts" module>
	export interface Props {
		suggestions: string[] | undefined;
		selectSuggestion?: (suggestion: string) => void;
		item?: Snippet<[string]>;
	}
</script>

<script lang="ts">
	import { setPosition } from '@gitbutler/ui/utils/tooltipPosition';
	import { flyScale } from '@gitbutler/ui/utils/transitions';
	import type { Snippet } from 'svelte';

	const { suggestions, selectSuggestion, item }: Props = $props();

	let selectedSuggestionIndex = $state<number>();
	let targetEl: HTMLElement | undefined = $state();

	$effect(() => {
		if (suggestions) {
			// Reset the selected suggestion index when the suggestions change
			selectedSuggestionIndex = undefined;
		}
	});

	export function onArrowUp() {
		if (suggestions === undefined) return;

		if (selectedSuggestionIndex === undefined) {
			selectedSuggestionIndex = suggestions.length - 1;
			return;
		}

		selectedSuggestionIndex =
			(selectedSuggestionIndex - 1 + suggestions.length) % suggestions.length;
	}

	export function onArrowDown() {
		if (suggestions === undefined) return;

		if (selectedSuggestionIndex === undefined) {
			selectedSuggestionIndex = 0;
			return;
		}

		selectedSuggestionIndex = (selectedSuggestionIndex + 1) % suggestions.length;
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
					{:else}
						{#each suggestions as suggestion, idx}
							<li>
								<div class="suggestion-item" class:selected={idx === selectedSuggestionIndex}>
									<button type="button" onclick={() => selectSuggestion?.(suggestion)}>
										{#if item}
											{@render item(suggestion)}
										{:else}
											<p class="text-12 text-semibold name truncate">
												{suggestion}
											</p>
										{/if}
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

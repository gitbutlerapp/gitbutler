<script lang="ts">
	import FilterPill from './FilterPill.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import ScrollableContainer from '$lib/shared/ScrollableContainer.svelte';
	import SelectItem from '$lib/shared/SelectItem.svelte';
	import { pxToRem } from '$lib/utils/pxToRem';
	import {
		formatFilterName,
		suggestionIsApplied,
		type AppliedFilter,
		type FilterDescription,
		type FilterSuggestion
	} from '$lib/vbranches/filtering';

	const maxPadding = 10;

	interface Props {
		maxHeight?: number;
		searchBarWrapper: HTMLElement;
		handleSuggestionClick: (suggestion: FilterSuggestion) => void;
		filterDescriptions: FilterDescription[] | undefined;
		appliedFilters: AppliedFilter[] | undefined;
		value: string | undefined;
	}

	let {
		maxHeight = 260,
		searchBarWrapper,
		handleSuggestionClick,
		value,
		appliedFilters,
		filterDescriptions
	}: Props = $props();

	let listOpen = $state<boolean>(false);
	let highlightIndex = $state<number | undefined>(undefined);
	let suggestions = $derived<FilterSuggestion[] | undefined>(
		filterDescriptions
			?.flatMap((d) => d.suggestions ?? [])
			?.filter((s) => {
				if (value && !s.name.startsWith(value)) return false;
				if (appliedFilters !== undefined && suggestionIsApplied(s, appliedFilters)) return false;
				return true;
			})
	);

	function setMaxHeight() {
		if (maxHeight) return;
		if (!searchBarWrapper) return;
		maxHeight = window.innerHeight - searchBarWrapper.getBoundingClientRect().bottom - maxPadding;
	}

	export function isOpen() {
		return listOpen;
	}

	export function openList() {
		setMaxHeight();
		listOpen = true;
	}

	export function closeList() {
		listOpen = false;
		highlightIndex = undefined;
	}

	export function arrowUp() {
		if (!suggestions?.length) return;
		if (highlightIndex === undefined) {
			highlightIndex = suggestions.length - 1;
		} else {
			highlightIndex = highlightIndex === 0 ? suggestions.length - 1 : highlightIndex - 1;
		}
	}

	export function arrowDown() {
		if (!suggestions?.length) return;
		if (highlightIndex === undefined) {
			highlightIndex = 0;
		} else {
			highlightIndex = highlightIndex === suggestions.length - 1 ? 0 : highlightIndex + 1;
		}
	}

	export function enter(): boolean {
		if (highlightIndex === undefined || !suggestions?.length) return false;

		handleSuggestionClick(suggestions[highlightIndex]);
		highlightIndex = undefined;
		return true;
	}

	function isHighlighted(suggestion: FilterSuggestion) {
		if (highlightIndex === undefined || !suggestions?.length) return false;
		return suggestion === suggestions[highlightIndex];
	}
</script>

{#if suggestions?.length}
	<div
		class="options card"
		style:display={listOpen ? undefined : 'none'}
		style:max-height={maxHeight && pxToRem(maxHeight)}
		use:clickOutside={{
			trigger: searchBarWrapper,
			handler: closeList,
			enabled: listOpen
		}}
	>
		<ScrollableContainer initiallyVisible>
			<div class="options__group">
				{#each suggestions as suggestion}
					<div tabindex="-1" role="none">
						<SelectItem
							selected={false}
							highlighted={isHighlighted(suggestion)}
							on:click={() => handleSuggestionClick(suggestion)}
						>
							<div class="filter-suggestion">
								<FilterPill name={formatFilterName(suggestion)} value={suggestion.value} />
								<span class="description">
									{suggestion.description}
								</span>
							</div>
						</SelectItem>
					</div>
				{/each}
			</div>
		</ScrollableContainer>
	</div>
{/if}

<style lang="postcss">
	.options {
		position: absolute;
		right: 0;
		top: 100%;
		width: 100%;
		z-index: var(--z-floating);
		margin-top: 4px;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-s);
		overflow: hidden;
	}

	.options__group {
		display: flex;
		flex-direction: column;
		padding: 6px;
		gap: 2px;

		&:not(&:first-child):last-child {
			border-top: 1px solid var(--clr-border-2);
		}
	}

	.filter-suggestion {
		display: flex;
		align-items: center;
		gap: 6px;

		& .description {
			color: var(--clr-scale-ntrl-50);
		}
	}
</style>

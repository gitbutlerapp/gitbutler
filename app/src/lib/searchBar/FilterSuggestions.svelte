<script lang="ts">
	import FilterPill from './FilterPill.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import ScrollableContainer from '$lib/shared/ScrollableContainer.svelte';
	import SelectItem from '$lib/shared/SelectItem.svelte';
	import { pxToRem } from '$lib/utils/pxToRem';
	import {
		formatFilterName,
		getSuggestionDescription,
		suggestionIsApplied,
		tryToParseFilter,
		type AppliedFilter,
		type DynamicFilterSuggestion,
		type FilterDescription,
		type FilterSuggestion,

		type StaticFilterSuggestion

	} from '$lib/vbranches/filtering';

	const MAX_PADDING = 10;
	const MAX_FILTER_SPECIFIC_SUGGESTIONS = 10;

	interface Props {
		maxHeight?: number;
		searchBarWrapper: HTMLElement;
		handleSuggestionClick: (suggestion: FilterSuggestion) => void;
		filterDescriptions: FilterDescription[] | undefined;
		appliedFilters: AppliedFilter[] | undefined;
		value: string | undefined;
	}

	let {
		maxHeight,
		searchBarWrapper,
		handleSuggestionClick,
		value,
		appliedFilters,
		filterDescriptions
	}: Props = $props();

	let listOpen = $state<boolean>(false);
	let highlightIndex = $state<number | undefined>(undefined);
	let suggestions = $derived<StaticFilterSuggestion[] | undefined>(
		filterDescriptions
			?.flatMap((d) => d.suggestions ?? [])
			?.filter((s) => {
				if (value && !s.name.startsWith(value)) return false;
				if (appliedFilters !== undefined && suggestionIsApplied(s, appliedFilters)) return false;
				return true;
			})
	);

	let filterSpecificSuggestions = $derived<DynamicFilterSuggestion[] | undefined>(
		value
			? filterDescriptions
					?.filter((d) => value?.startsWith(formatFilterName(d)))
					.flatMap((d) => d.dynamicSuggestions ?? [])
					.filter((s) => appliedFilters === undefined || !suggestionIsApplied(s, appliedFilters))
					.filter((s) => {
						const f = tryToParseFilter(value);
						if (!f?.values.length) return true;
						return f.values.some((v) => s.value?.includes(v));
					})
					.slice(undefined, MAX_FILTER_SPECIFIC_SUGGESTIONS)
			: undefined
	);

	let list = $derived<FilterSuggestion[] | undefined>(
		suggestions?.length ? suggestions : filterSpecificSuggestions
	);

	function setMaxHeight() {
		if (maxHeight) return;
		if (!searchBarWrapper) return;
		maxHeight = window.innerHeight - searchBarWrapper.getBoundingClientRect().bottom - MAX_PADDING;
	}

	export function isOpen() {
		return listOpen;
	}

	export function openList() {
		setMaxHeight();
		listOpen = true;
		highlightIndex = undefined;
	}

	export function closeList() {
		listOpen = false;
		highlightIndex = undefined;
	}

	export function arrowUp() {
		if (!list?.length) return;
		if (highlightIndex === undefined) {
			highlightIndex = list.length - 1;
		} else {
			highlightIndex = highlightIndex === 0 ? list.length - 1 : highlightIndex - 1;
		}
	}

	export function arrowDown() {
		if (!list?.length) return;
		if (highlightIndex === undefined) {
			highlightIndex = 0;
		} else {
			highlightIndex = highlightIndex === list.length - 1 ? 0 : highlightIndex + 1;
		}
	}

	export function enter(): boolean {
		if (highlightIndex === undefined || !list?.length) return false;

		handleSuggestionClick(list[highlightIndex]);
		highlightIndex = undefined;
		return true;
	}

	function isHighlighted(suggestion: FilterSuggestion) {
		if (highlightIndex === undefined || !list?.length) return false;
		return suggestion === list[highlightIndex];
	}
</script>

{#if list?.length}
	<div
		class="options card"
		style:display={listOpen ? undefined : 'none'}
		style:max-height={pxToRem(maxHeight)}
		use:clickOutside={{
			trigger: searchBarWrapper,
			handler: closeList,
			enabled: listOpen
		}}
	>
		<ScrollableContainer initiallyVisible>
			<div class="options__group">
				{#each list as suggestion}
					<div tabindex="-1" role="none">
						<SelectItem
							selected={false}
							highlighted={isHighlighted(suggestion)}
							on:click={() => handleSuggestionClick(suggestion)}
						>
							<div class="filter-suggestion">
								<FilterPill name={formatFilterName(suggestion)} value={suggestion.value} />
								<span class="description">
									{getSuggestionDescription(suggestion)}
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

<script lang="ts">
	import { clickOutside } from '$lib/clickOutside';
	import { pxToRem } from '$lib/utils/pxToRem';
	import {
		DEFAULT_FILTER_SUGGESTIONS,
		formatFilterName,
		type FilterSuggestion
	} from '$lib/vbranches/filtering';
	import ScrollableContainer from '../ScrollableContainer.svelte';
	import SelectItem from '../SelectItem.svelte';
	import FilterPill from './FilterPill.svelte';

	const maxPadding = 10;

	interface Props {
		maxHeight?: number;
		searchBarWrapper: HTMLElement;
		handleSuggestionClick: (suggestion: FilterSuggestion) => void;
	}

	let { maxHeight = 260, searchBarWrapper, handleSuggestionClick }: Props = $props();

	let listOpen = $state<boolean>(false);

	function setMaxHeight() {
		if (maxHeight) return;
		if (!searchBarWrapper) return;
		maxHeight = window.innerHeight - searchBarWrapper.getBoundingClientRect().bottom - maxPadding;
	}

	export function openList() {
		setMaxHeight();
		listOpen = true;
	}

	export function closeList() {
		listOpen = false;
	}
</script>

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
			{#each DEFAULT_FILTER_SUGGESTIONS as suggestion}
				<div tabindex="-1" role="none">
					<SelectItem
						selected={false}
						highlighted={false}
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

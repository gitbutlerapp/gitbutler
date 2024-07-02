<script lang="ts">
	import FilterPillContainer from './FilterPillContainer.svelte';
	import FilterSuggestions from './FilterSuggestions.svelte';
	import { getFilterContext } from './filterContext.svelte';
	import Icon from '$lib/shared/Icon.svelte';
	import { KeyName } from '$lib/utils/hotkeys';
	import { isChar } from '$lib/utils/string';
	import {
		formatFilterName,
		parseFilterValues,
		type FilterDescription,
		type FilterSuggestion
	} from '$lib/vbranches/filtering';
	import type iconsJson from '$lib/icons/icons.json';

	interface Props {
		placeholder?: string;
		icon?: keyof typeof iconsJson;
		filterDescriptions?: FilterDescription[];
		onFocus?: () => void;
	}

	let { filterDescriptions, placeholder, icon, onFocus }: Props = $props();

	const filterContext = getFilterContext();

	let searchBarWrapper = $state<HTMLElement | undefined>(undefined);
	let searchBarInput = $state<HTMLInputElement | undefined>(undefined);
	let filterSuggestionElem = $state<FilterSuggestions | undefined>(undefined);

	function getFilterDescFromValue(desc: FilterDescription[]): FilterDescription | undefined {
		if (!filterContext.searchQuery) return undefined;
		const filterDesc = desc.find((d) => filterContext.searchQuery?.startsWith(formatFilterName(d)));
		return filterDesc;
	}

	function getAllowedFilterValue(filterDesc: FilterDescription): string[] | undefined {
		if (!filterContext.searchQuery) return undefined;
		return parseFilterValues(filterContext.searchQuery, filterDesc);
	}

	function handleSuggestionClick(suggestion: FilterSuggestion) {
		const filterDesc = filterDescriptions?.find((f) => f.name === suggestion.name);
		if (!filterDesc) return;
		if (suggestion.value === undefined) {
			filterContext.searchQuery = formatFilterName(filterDesc);
			searchBarInput?.focus();
			return;
		}
		filterContext.addFilter({ name: filterDesc.name, values: [suggestion.value] });
		filterContext.searchQuery = undefined;
	}

	function handleEnter() {
		// If there is a highlighted item, select it
		if (filterSuggestionElem?.enter()) return;

		if (!filterDescriptions) return;
		const filterDesc = getFilterDescFromValue(filterDescriptions);
		if (!filterDesc) return;
		const filterValue = getAllowedFilterValue(filterDesc);
		if (!filterValue) return;
		filterContext.addFilter({ name: filterDesc.name, values: filterValue });
		filterContext.searchQuery = undefined;
	}

	function handleDelete() {
		filterSuggestionElem?.openList();
		if (!filterDescriptions || !filterContext.appliedFilters || filterContext.searchQuery) return;
		filterContext.popFilter();
	}

	function handleArrowUp(e: KeyboardEvent) {
		if (filterSuggestionElem?.isOpen()) {
			filterSuggestionElem?.arrowUp();
			e.stopPropagation();
			e.preventDefault();
		}
	}

	function handleArrowDown(e: KeyboardEvent) {
		if (filterSuggestionElem?.isOpen()) {
			filterSuggestionElem?.arrowDown();
			e.stopPropagation();
			e.preventDefault();
		}
	}

	function handleEscape() {
		if (filterSuggestionElem?.isOpen()) {
			filterSuggestionElem?.closeList();
			return;
		}
		searchBarInput?.blur();
	}

	function handleChar() {
		if (!filterDescriptions || !filterContext.searchQuery) return;
		filterSuggestionElem?.openList();
	}

	function onkeydown(e: KeyboardEvent) {
		const { key } = e;

		switch (key) {
			case KeyName.Enter:
				handleEnter();
				break;
			case KeyName.Delete:
				handleDelete();
				break;
			case KeyName.Up:
				handleArrowUp(e);
				break;
			case KeyName.Down:
				handleArrowDown(e);
				break;
			case KeyName.Escape:
				handleEscape();
				break;
			default:
				if (isChar(key)) handleChar();
				break;
		}
	}

	function onfocus() {
		onFocus?.();
		filterSuggestionElem?.openList();
	}
</script>

<div class="search-bar-wrapper" bind:this={searchBarWrapper}>
	<div class="textbox text-input">
		{#if icon}
			<div class="textbox__icon">
				<Icon name={icon} size={24} />
			</div>
		{/if}

		{#if filterContext.appliedFilters.length}
			<FilterPillContainer
				appliedFilters={filterContext.appliedFilters}
				handleFilterClick={(f) => filterContext.removeFilter(f)}
			/>
		{/if}

		<input
			type="text"
			autocorrect="off"
			autocomplete="off"
			{placeholder}
			class="textbox__input text-base-18"
			bind:value={filterContext.searchQuery}
			bind:this={searchBarInput}
			{onkeydown}
			{onfocus}
		/>
	</div>

	{#if filterDescriptions?.length}
		<FilterSuggestions
			bind:this={filterSuggestionElem}
			{searchBarWrapper}
			{handleSuggestionClick}
			appliedFilters={filterContext.appliedFilters}
			{filterDescriptions}
			value={filterContext.searchQuery}
		/>
	{/if}
</div>

<style lang="postcss">
	.search-bar-wrapper {
		z-index: var(--z-floating);
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 6px;
		box-shadow: var(--fx-shadow-s);

		&:focus-within .textbox__icon {
			color: var(--clr-scale-ntrl-0);
		}
	}
	.textbox {
		display: flex;
		align-items: center;
	}

	.textbox__icon {
		pointer-events: none;
		color: var(--clr-scale-ntrl-50);
		margin-right: 10px;
	}

	.textbox__input {
		position: relative;
		flex-grow: 1;
		width: 100%;
		padding-top: 16px;
		padding-bottom: 16px;
		color: var(--clr-scale-ntrl-0);
		background-color: var(--clr-bg-1);
		outline: none;
	}
</style>

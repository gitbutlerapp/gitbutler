<script lang="ts">
	import type iconsJson from '$lib/icons/icons.json';
	import Icon from './Icon.svelte';
	import { KeyName } from '$lib/utils/hotkeys';
	import {
		formatFilterName,
		parseFilterValues,
		type AppliedFilter,
		type FilterDescription,
		type FilterSuggestion
	} from '$lib/vbranches/filtering';
	import FilterPillContainer from './SearchBar/FilterPillContainer.svelte';
	import FilterSuggestions from './SearchBar/FilterSuggestions.svelte';

	interface Props {
		value: string | undefined;
		placeholder?: string;
		icon?: keyof typeof iconsJson;
		filterDescriptions?: FilterDescription[];
		appliedFilters?: AppliedFilter[];
	}

	let {
		value = $bindable(),
		appliedFilters = $bindable(),
		filterDescriptions,
		placeholder,
		icon
	}: Props = $props();

	let searchBarWrapper = $state<HTMLElement | undefined>(undefined);
	let searchBarInput = $state<HTMLInputElement | undefined>(undefined);
	let filterSuggestionElem = $state<FilterSuggestions | undefined>(undefined);

	function getFilterDescFromValue(desc: FilterDescription[]): FilterDescription | undefined {
		if (!value) return undefined;
		const filterDesc = desc.find((d) => value?.startsWith(`${d.name}:`));
		return filterDesc;
	}

	function getAllowedFilterValue(filterDesc: FilterDescription): string[] | undefined {
		if (!value) return undefined;
		return parseFilterValues(value, filterDesc);
	}

	function applyFilter(filterDesc: FilterDescription, filterValue: string[]) {
		if (!filterValue || appliedFilters === undefined) return;
		appliedFilters = [...appliedFilters, { name: filterDesc.name, values: filterValue }];
	}

	function handleSuggestionClick(suggestion: FilterSuggestion) {
		const filterDesc = filterDescriptions?.find((f) => f.name === suggestion.name);
		if (!filterDesc) return;
		if (suggestion.value === undefined) {
			value = formatFilterName(filterDesc);
			searchBarInput?.focus();
			filterSuggestionElem?.closeList();
			return;
		}
		applyFilter(filterDesc, [suggestion.value]);
	}

	function handleEnter() {
		if (!filterDescriptions || appliedFilters === undefined) return;
		const filterDesc = getFilterDescFromValue(filterDescriptions);
		if (!filterDesc) return;
		const filterValue = getAllowedFilterValue(filterDesc);
		if (!filterValue) return;
		applyFilter(filterDesc, filterValue);
		value = undefined;
	}

	function handleDelete() {
		if (value) {
			if (value.length === 1) {
				filterSuggestionElem?.openList();
			}
      return;
		}
		if (!filterDescriptions || !appliedFilters) return;
		appliedFilters = appliedFilters.slice(0, -1);
	}

	function handleEscape() {
		searchBarInput?.blur();
		filterSuggestionElem?.closeList();
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
			case KeyName.Escape:
				handleEscape();
				break;
			default:
				break;
		}
	}
</script>

<div class="search-bar-wrapper" bind:this={searchBarWrapper}>
	<div class="textbox text-input">
		{#if icon}
			<div class="textbox__icon">
				<Icon name={icon} size={24} />
			</div>
		{/if}

		{#if appliedFilters?.length}
			<FilterPillContainer {appliedFilters} />
		{/if}

		<input
			type="text"
			autocorrect="off"
			autocomplete="off"
			{placeholder}
			class="textbox__input text-base-18"
			bind:value
			bind:this={searchBarInput}
			oninput={(e) => {
				value = e.currentTarget.value;
			}}
			{onkeydown}
			onfocus={() => filterSuggestionElem?.openList()}
		/>
	</div>

	{#if filterDescriptions?.length}
		<FilterSuggestions
			bind:this={filterSuggestionElem}
			{searchBarWrapper}
			{handleSuggestionClick}
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

<script lang="ts">
	import type iconsJson from '$lib/icons/icons.json';
	import Icon from './Icon.svelte';
	import { KeyName } from '$lib/utils/hotkeys';
	import type { AppliedFilter, FilterDescription } from '$lib/vbranches/filtering';
	import FilterPillContainer from './SearchBar/FilterPillContainer.svelte';
	import ScrollableContainer from './ScrollableContainer.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import { pxToRem } from '$lib/utils/pxToRem';
	import SelectItem from './SelectItem.svelte';

	const maxPadding = 10;

	interface FilterSuggestion {
		name: string;
		value?: string;
	}

	interface Props {
		value: string | undefined;
		placeholder?: string;
		icon?: keyof typeof iconsJson;
		filterDescriptions?: FilterDescription[];
		appliedFilters?: AppliedFilter[];
		maxHeight?: number;
	}

	let {
		value = $bindable(),
		appliedFilters = $bindable(),
		filterDescriptions,
		placeholder,
		icon,
		maxHeight = 260
	}: Props = $props();

	let searchBarWrapper = $state<HTMLElement | undefined>(undefined);
	let searchBarInput = $state<HTMLInputElement | undefined>(undefined);
	let listOpen = $state<boolean>(false);
	const filterSuggestions: FilterSuggestion[] =
		filterDescriptions?.flatMap((f) => {
			if (f.allowedValues) {
				return f.allowedValues.map((v) => ({ name: f.name, value: v }));
			}
			return [{ name: f.name }];
		}) ?? [];

	function setMaxHeight() {
		if (maxHeight) return;
		if (!searchBarWrapper) return;
		maxHeight = window.innerHeight - searchBarWrapper.getBoundingClientRect().bottom - maxPadding;
	}

	function openList() {
		setMaxHeight();
		listOpen = true;
	}

	function closeList() {
		listOpen = false;
	}

	function getFilterDescFromValue(desc: FilterDescription[]): FilterDescription | undefined {
		if (!value) return undefined;
		const filterDesc = desc.find((d) => value?.startsWith(`${d.name}:`));
		return filterDesc;
	}

	function getAllowedFilterValue(filterDesc: FilterDescription): string[] | undefined {
		if (!value) return undefined;
		const filterValue = value.replace(`${filterDesc.name}:`, '').split(',');
		if (
			filterDesc.allowedValues === undefined ||
			filterValue.every((v) => filterDesc.allowedValues?.includes(v))
		) {
			return filterValue;
		}
		return undefined;
	}

	function applyFilter(filterDesc: FilterDescription, filterValue: string[]) {
		if (!filterValue || appliedFilters === undefined) return;
		appliedFilters = [...appliedFilters, { name: filterDesc.name, values: filterValue }];
	}

	function handleSuggestionClick(suggestion: FilterSuggestion) {
		const filterDesc = filterDescriptions?.find((f) => f.name === suggestion.name);
		if (!filterDesc) return;
		if (suggestion.value === undefined) {
			value = `${filterDesc.name}:`;
			searchBarInput?.focus();
			closeList();
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
		if (!filterDescriptions || !appliedFilters || value) return;
		appliedFilters = appliedFilters.slice(0, -1);
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
			onfocus={openList}
		/>
	</div>
	{#if filterSuggestions.length}
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
					{#each filterSuggestions as suggestion}
						<div tabindex="-1" role="none">
							<SelectItem
								selected={false}
								highlighted={false}
								on:click={() => handleSuggestionClick(suggestion)}
							>
								{suggestion.name + (suggestion.value ? `:${suggestion.value}` : '')}
							</SelectItem>
						</div>
					{/each}
				</div>
			</ScrollableContainer>
		</div>
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
</style>

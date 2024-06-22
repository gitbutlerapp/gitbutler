<script lang="ts">
	import type iconsJson from '$lib/icons/icons.json';
	import Icon from './Icon.svelte';
	import { KeyName } from '$lib/utils/hotkeys';
	import type { AppliedFilter, FilterDescription } from '$lib/vbranches/filtering';

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

	function getFilterDescFromValue(desc: FilterDescription[]): FilterDescription | undefined {
		if (!value) return undefined;
		const filterDesc = desc.find((d) => value?.startsWith(`${d.name}:`));
		return filterDesc;
	}

	function getAllowedFilterValue(filterDesc: FilterDescription): string[] | undefined {
		if (!value) return undefined;
		const filterValue = value.replace(`${filterDesc.name}:`, '');
		if (filterDesc.allowedValues?.includes(filterValue) ?? true) {
			return filterValue.split(',');
		}
		return undefined;
	}

	function handleEnter() {
		if (!filterDescriptions || appliedFilters === undefined) return;
		const filterDesc = getFilterDescFromValue(filterDescriptions);
		if (!filterDesc) return;
		const filterValue = getAllowedFilterValue(filterDesc);
		if (!filterValue) return;
		appliedFilters = [...appliedFilters, { name: filterDesc.name, values: filterValue }];
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

<div class="textbox text-input">
	{#if icon}
		<div class="textbox__icon">
			<Icon name={icon} size={24} />
		</div>
	{/if}

	{#if appliedFilters?.length}
		<div class="filter-pill-container">
			{#each appliedFilters as filter}
				<div class="filter-pill text-base-14">
					<div class="filter-name-prop">{filter.name}:</div>
					<div class="filter-name-value">{filter.values.join(',')}</div>
				</div>
			{/each}
		</div>
	{/if}

	<input
		type="text"
		autocorrect="off"
		autocomplete="off"
		{placeholder}
		class="textbox__input text-base-18"
		bind:value
		oninput={(e) => {
			value = e.currentTarget.value;
		}}
		{onkeydown}
	/>
</div>

<style lang="postcss">
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

	.filter-pill-container {
		margin-right: 10px;
		display: flex;
		gap: 4px;
	}

	.filter-pill {
		box-sizing: border-box;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s);
		background-color: var(--clr-scale-ntrl-70);
		display: flex;
		padding: 4px;
	}

	.filter-name-prop {
		color: var(--clr-scale-ntrl-0);
	}
</style>

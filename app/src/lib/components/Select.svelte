<script lang="ts" context="module">
	export type Selectable<S extends string> = Record<S, unknown>;
</script>

<script lang="ts" generics="Selectable extends Record<string, unknown>">
	import ScrollableContainer from './ScrollableContainer.svelte';
	import TextBox from './TextBox.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import { KeyName } from '$lib/utils/hotkeys';
	import { throttle } from '$lib/utils/misc';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { isChar, isStr } from '$lib/utils/string';
	import { createEventDispatcher } from 'svelte';

	const INPUT_THROTTLE_TIME = 100;

	type SelectableKey = keyof Selectable;

	export let id: undefined | string = undefined;
	export let label = '';
	export let disabled = false;
	export let loading = false;
	export let wide = false;
	export let items: Selectable[];
	export let labelId: SelectableKey = 'label';
	export let itemId: SelectableKey = 'value';
	export let value: any = undefined;
	export let selectedItemId: any = undefined;
	export let placeholder = '';
	export let maxHeight: number | undefined = 260;

	$: if (selectedItemId) value = items.find((item) => item[itemId] === selectedItemId);

	const dispatch = createEventDispatcher<{ select: { value: any } }>();
	const maxPadding = 10;

	let listOpen = false;
	let element: HTMLElement;
	let options: HTMLDivElement;
	let highlightIndex: number | undefined = undefined;
	let highlightedItem: Selectable | undefined = undefined;
	let filterText: string | undefined = undefined;
	let filteredItems: Selectable[] = items;

	const filterItems = throttle((items: Selectable[], filterText: string | undefined) => {
		if (!filterText) {
			return items;
		}

		return items.filter((it) => {
			const property = it[labelId];
			if (!isStr(property)) return false;
			return property.includes(filterText);
		});
	}, INPUT_THROTTLE_TIME);

	$: filteredItems = filterItems(items, filterText);

	$: highlightedItem = highlightIndex !== undefined ? filteredItems[highlightIndex] : undefined;

	function handleItemClick(item: Selectable) {
		if (item?.selectable === false) return;
		if (value && value[itemId] === item[itemId]) return closeList();
		selectedItemId = item[itemId];
		dispatch('select', { value });
		closeList();
	}
	function setMaxHeight() {
		if (maxHeight) return;
		maxHeight = window.innerHeight - element.getBoundingClientRect().bottom - maxPadding;
	}

	function toggleList() {
		if (listOpen) closeList();
		else openList();
	}

	function openList() {
		setMaxHeight();
		listOpen = true;
	}

	function closeList() {
		listOpen = false;
		highlightIndex = undefined;
		filterText = undefined;
	}

	function handleEnter() {
		if (highlightIndex !== undefined) {
			handleItemClick(filteredItems[highlightIndex]);
		}
		closeList();
	}

	function handleArrowUp() {
		if (filteredItems.length === 0) return;
		if (highlightIndex === undefined) {
			highlightIndex = filteredItems.length - 1;
		} else {
			highlightIndex = highlightIndex === 0 ? filteredItems.length - 1 : highlightIndex - 1;
		}
	}

	function handleArrowDown() {
		if (filteredItems.length === 0) return;
		if (highlightIndex === undefined) {
			highlightIndex = 0;
		} else {
			highlightIndex = highlightIndex === filteredItems.length - 1 ? 0 : highlightIndex + 1;
		}
	}

	function handleChar(char: string) {
		highlightIndex = undefined;
		filterText ??= '';
		filterText += char;
	}

	function handleDelete() {
		if (filterText === undefined) return;

		if (filterText.length === 1) {
			filterText = undefined;
			return;
		}

		filterText = filterText.slice(0, -1);
	}

	function handleKeyDown(e: CustomEvent<KeyboardEvent>) {
		if (!listOpen) {
			return;
		}
		e.detail.stopPropagation();
		e.detail.preventDefault();

		const { key } = e.detail;
		switch (key) {
			case KeyName.Escape:
				closeList();
				break;
			case KeyName.Up:
				handleArrowUp();
				break;
			case KeyName.Down:
				handleArrowDown();
				break;
			case KeyName.Enter:
				handleEnter();
				break;
			case KeyName.Delete:
				handleDelete();
				break;
			default:
				if (isChar(key)) handleChar(key);
				break;
		}
	}
</script>

<div class="select-wrapper" class:wide bind:this={element}>
	{#if label}
		<label for={id} class="select__label text-base-body-13 text-semibold">{label}</label>
	{/if}
	<TextBox
		{id}
		{placeholder}
		noselect
		readonly
		type="select"
		reversedDirection
		icon="select-chevron"
		value={filterText ?? value?.[labelId]}
		disabled={disabled || loading}
		on:mousedown={() => toggleList()}
		on:keydown={(ev) => handleKeyDown(ev)}
	/>
	<div
		class="options card"
		style:display={listOpen ? undefined : 'none'}
		bind:this={options}
		style:max-height={maxHeight && pxToRem(maxHeight)}
		use:clickOutside={{
			trigger: element,
			handler: closeList,
			enabled: listOpen
		}}
	>
		<ScrollableContainer initiallyVisible>
			{#if filteredItems}
				<div class="options__group">
					{#each filteredItems as item}
						<div
							class="option"
							class:selected={item === value}
							tabindex="-1"
							role="none"
							on:mousedown={() => handleItemClick(item)}
							on:keydown|preventDefault|stopPropagation
						>
							<slot
								name="template"
								{item}
								selected={item === value}
								highlighted={item === highlightedItem}
							/>
						</div>
					{/each}
				</div>
			{/if}
			{#if $$slots?.append}
				<div class="options__group">
					<slot name="append" />
				</div>
			{/if}
		</ScrollableContainer>
	</div>
</div>

<style lang="postcss">
	.select-wrapper {
		/* display set directly on element */
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.select__label {
		text-align: left;
		color: var(--clr-scale-ntrl-50);
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

	.wide {
		width: 100%;
	}
</style>

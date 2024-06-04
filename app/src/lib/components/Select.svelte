<script lang="ts" context="module">
	export type SelectItemType<S extends string> = Record<S, unknown>;
</script>

<script lang="ts" generics="SelectItemType extends Record<string, unknown>">
	import ScrollableContainer from './ScrollableContainer.svelte';
	import TextBox from './TextBox.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import { filterStringByKey } from '$lib/utils/filters';
	import { KeyName } from '$lib/utils/hotkeys';
	import { throttle } from '$lib/utils/misc';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { isChar } from '$lib/utils/string';
	import { createEventDispatcher } from 'svelte';

	const INPUT_THROTTLE_TIME = 100;

	type SelectItemKeyType = keyof SelectItemType;

	export let id: undefined | string = undefined;
	export let label = '';
	export let disabled = false;
	export let loading = false;
	export let wide = false;
	export let items: SelectItemType[];
	export let labelId: SelectItemKeyType = 'label';
	export let itemId: SelectItemKeyType = 'value';
	export let value: any = undefined;
	export let selectedItemId: any = undefined;
	export let placeholder = '';
	export let maxHeight: number | undefined = 260;

	$: if (selectedItemId) value = items.find((item) => item[itemId] === selectedItemId);

	const SLOTS = $$props.$$slots;
	const dispatch = createEventDispatcher<{ select: { value: any } }>();
	const maxPadding = 10;

	let listOpen = false;
	let element: HTMLElement;
	let options: HTMLDivElement;
	let highlightIndex: number | undefined = undefined;
	let highlightedItem: SelectItemType | undefined = undefined;
	let filterText: string | undefined = undefined;
	let filteredItems: SelectItemType[] = items;

	$: filterText === undefined
		? (filteredItems = items)
		: (filteredItems = filterStringByKey(items, labelId, filterText));

	// Set highlighted item based on index
	$: highlightIndex !== undefined
		? (highlightedItem = filteredItems[highlightIndex])
		: (highlightedItem = undefined);

	function handleItemClick(item: SelectItemType) {
		if (item?.selectable === false) return;
		if (value && value[itemId] === item[itemId]) return closeList();
		selectedItemId = item[itemId];
		dispatch('select', { value });
		listOpen = false;
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
		if (highlightIndex === undefined) {
			highlightIndex = filteredItems.length - 1;
		} else {
			highlightIndex = highlightIndex === 0 ? filteredItems.length - 1 : highlightIndex - 1;
		}
	}

	function handleArrowDown() {
		if (highlightIndex === undefined) {
			highlightIndex = 0;
		} else {
			highlightIndex = highlightIndex === filteredItems.length - 1 ? 0 : highlightIndex + 1;
		}
	}

	const handleChar = throttle((char: string) => {
		highlightIndex = undefined;
		filterText ??= '';
		filterText += char;
	}, INPUT_THROTTLE_TIME);

	const handleDelete = throttle(() => {
		if (filterText === undefined) return;

		if (filterText.length === 1) {
			filterText = undefined;
			return;
		}

		filterText = filterText.slice(0, -1);
	}, INPUT_THROTTLE_TIME);

	function handleKeyDown(event: CustomEvent<KeyboardEvent>) {
		if (!listOpen) {
			return;
		}
		event.detail.stopPropagation();
		event.detail.preventDefault();

		const { key } = event.detail;
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
			{#if SLOTS?.append}
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

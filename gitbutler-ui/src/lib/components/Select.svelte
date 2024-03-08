<script lang="ts">
	import ScrollableContainer from './ScrollableContainer.svelte';
	import TextBox from './TextBox.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import { createEventDispatcher } from 'svelte';

	export let id: undefined | string = undefined;
	export let label = '';
	export let disabled = false;
	export let loading = false;
	export let wide = false;
	export let items: any[];
	export let labelId = 'label';
	export let itemId = 'value';
	export let value: any = undefined;
	export let placeholder = '';

	const SLOTS = $$props.$$slots;
	const dispatch = createEventDispatcher<{ select: { value: any } }>();
	const maxPadding = 10;

	let listOpen = false;
	let element: HTMLElement;
	let options: HTMLDivElement;
	let maxHeight = 200;

	function handleItemClick(item: any) {
		if (item?.selectable === false) return;
		if (value && value[itemId] === item[itemId]) return closeList();
		value = item;
		dispatch('select', { value });
		listOpen = false;
	}

	function scrollIntoView() {
		const selected = element.querySelector('.selected');
		if (selected) selected.scrollIntoView();
	}

	function setMaxHeight() {
		maxHeight = window.innerHeight - element.getBoundingClientRect().bottom - maxPadding;
	}

	function toggleList() {
		if (listOpen) closeList();
		else openList();
	}

	function openList() {
		setMaxHeight();
		listOpen = true;
		setTimeout(() => scrollIntoView(), 50);
	}

	function closeList() {
		listOpen = false;
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
		value={value?.[labelId]}
		disabled={disabled || loading}
		on:mousedown={() => toggleList()}
	/>
	<div
		class="options card"
		style:display={listOpen ? undefined : 'none'}
		bind:this={options}
		style:max-height={`${maxHeight}px`}
		use:clickOutside={{
			trigger: element,
			handler: () => (listOpen = !listOpen),
			enabled: listOpen
		}}
	>
		<ScrollableContainer initiallyVisible>
			{#if items}
				<div class="options__group">
					{#each items as item}
						<div
							class="option"
							class:selected={item == value}
							tabindex="-1"
							role="none"
							on:mousedown={() => handleItemClick(item)}
							on:keydown|preventDefault|stopPropagation
						>
							<slot name="template" {item} selected={item == value} />
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
		gap: var(--space-6);
	}

	.select__label {
		text-align: left;
		color: var(--clr-theme-scale-ntrl-50);
	}

	.options {
		position: absolute;
		right: 0;
		top: 100%;
		width: 100%;
		z-index: 50;
		margin-top: var(--space-4);
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-theme-container-outline-light);
		background: var(--clr-theme-container-light);
		box-shadow: var(--fx-shadow-s);
	}

	.options__group {
		display: flex;
		flex-direction: column;
		padding: var(--space-6);
		gap: var(--space-2);

		&:not(&:first-child):last-child {
			border-top: 1px solid var(--clr-theme-container-outline-light);
		}
	}

	.wide {
		width: 100%;
	}
</style>

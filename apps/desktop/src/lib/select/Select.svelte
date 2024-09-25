<script lang="ts" module>
	export type SelectItem<T extends string = string> = {
		label: string;
		value: T;
		selectable?: boolean;
	};
</script>

<script lang="ts" generics="T extends string">
	import OptionsGroup from './OptionsGroup.svelte';
	import SearchItem from './SearchItem.svelte';
	import TextBox from '../shared/TextBox.svelte';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { KeyName } from '$lib/utils/hotkeys';
	import { resizeObserver } from '$lib/utils/resizeObserver';
	import { type Snippet } from 'svelte';

	interface SelectProps {
		id?: string;
		label?: string;
		disabled?: boolean;
		loading?: boolean;
		wide?: boolean;
		options: SelectItem<T>[];
		value?: T;
		placeholder?: string;
		maxHeight?: number;
		searchable?: boolean;
		itemSnippet: Snippet<[{ item: SelectItem<T>; highlighted: boolean }]>;
		children?: Snippet;
		onselect?: (value: T) => void;
	}

	const {
		id,
		label,
		disabled,
		loading,
		wide,
		options = [],
		value,
		placeholder,
		maxHeight,
		searchable,
		itemSnippet,
		children,
		onselect
	}: SelectProps = $props();

	let selectWrapperEl: HTMLElement;

	let highlightedIndex: number | undefined = $state(undefined);
	let searchValue = $state('');
	let filteredOptions = $derived(
		options.filter((item) => item.label.toLowerCase().includes(searchValue.toLowerCase()))
	);
	let maxHeightState = $state(maxHeight);
	let listOpen = $state(false);
	let inputBoundingRect = $state<DOMRect>();

	const maxBottomPadding = 20;

	function setMaxHeight() {
		if (maxHeight) return;
		maxHeightState =
			window.innerHeight - selectWrapperEl.getBoundingClientRect().bottom - maxBottomPadding;
	}

	function openList() {
		setMaxHeight();
		listOpen = true;
	}

	function closeList() {
		listOpen = false;
	}

	function clickOutside(e: MouseEvent) {
		if (e.target === e.currentTarget) closeList();
	}

	function getInputBoundingRect() {
		if (selectWrapperEl) {
			inputBoundingRect = selectWrapperEl.getBoundingClientRect();
		}
	}

	function toggleList() {
		getInputBoundingRect();

		if (listOpen) closeList();
		else openList();
	}

	function handleSelect(item: SelectItem<T>) {
		const value = item.value;
		onselect?.(value);
		closeList();
	}

	function handleEnter() {
		const option = highlightedIndex !== undefined ? filteredOptions[highlightedIndex] : undefined;
		if (option) {
			handleSelect(option);
		}
	}

	function handleArrowUp() {
		if (filteredOptions.length === 0) return;
		if (highlightedIndex === undefined) {
			highlightedIndex = filteredOptions.length - 1;
		} else {
			highlightedIndex = highlightedIndex === 0 ? filteredOptions.length - 1 : highlightedIndex - 1;
		}
	}

	function handleArrowDown() {
		if (filteredOptions.length === 0) return;
		if (highlightedIndex === undefined) {
			highlightedIndex = 0;
		} else {
			highlightedIndex = highlightedIndex === filteredOptions.length - 1 ? 0 : highlightedIndex + 1;
		}
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
		}
	}
</script>

<div class="select-wrapper" class:wide bind:this={selectWrapperEl}>
	{#if label}
		<label for={id} class="select__label text-13 text-body text-semibold">{label}</label>
	{/if}
	<TextBox
		{id}
		{placeholder}
		noselect
		readonly
		type="select"
		reversedDirection
		icon="select-chevron"
		value={options.find((item) => item.value === value)?.label || 'Select an option...'}
		disabled={disabled || loading}
		on:mousedown={toggleList}
		on:keydown={(ev) => handleKeyDown(ev)}
	/>
	{#if listOpen}
		<div
			role="presentation"
			class="overlay-wrapper"
			onclick={clickOutside}
			use:resizeObserver={() => {
				getInputBoundingRect();
				setMaxHeight();
			}}
		>
			<div
				class="options card"
				style:width="{inputBoundingRect?.width}px"
				style:top={inputBoundingRect?.top
					? `${inputBoundingRect.top + inputBoundingRect.height}px`
					: undefined}
				style:left={inputBoundingRect?.left ? `${inputBoundingRect.left}px` : undefined}
				style:max-height={maxHeightState && `${maxHeightState}px`}
			>
				<ScrollableContainer initiallyVisible>
					{#if searchable && options.length > 5}
						<SearchItem bind:searchValue />
					{/if}
					<OptionsGroup>
						{#if filteredOptions.length === 0}
							<div class="text-13 text-semibold option nothing-found">
								<span class=""> Nothing found ¯\_(ツ)_/¯ </span>
							</div>
						{/if}
						{#each filteredOptions as item, idx}
							<div class="option" tabindex="-1" role="none" onmousedown={() => handleSelect(item)}>
								{@render itemSnippet({ item, highlighted: idx === highlightedIndex })}
							</div>
						{/each}
					</OptionsGroup>

					{#if children}
						{@render children()}
					{/if}
				</ScrollableContainer>
			</div>
		</div>
	{/if}
</div>

<style lang="postcss">
	.select-wrapper {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.select__label {
		text-align: left;
		color: var(--clr-scale-ntrl-50);
	}

	.overlay-wrapper {
		z-index: var(--z-blocker);
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		/* background-color: rgba(0, 0, 0, 0.1); */
	}

	.options {
		user-select: none;
		position: absolute;
		z-index: var(--z-floating);
		margin-top: 4px;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-s);
		overflow: hidden;
		transform-origin: top;

		animation: fadeIn 0.16s ease-out forwards;
	}

	@keyframes fadeIn {
		0% {
			opacity: 0;
			transform: translateY(-6px);
		}
		40% {
			opacity: 1;
		}
		100% {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.wide {
		width: 100%;
	}

	.nothing-found {
		padding: 8px 8px;
		color: var(--clr-text-3);
	}
</style>

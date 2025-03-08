<script lang="ts" module>
	type T = string;

	export type SelectItem<T extends string = string> = {
		label: string;
		value: T;
		selectable?: boolean;
	};

	interface Props {
		id?: string;
		label?: string;
		disabled?: boolean;
		loading?: boolean;
		wide?: boolean;
		maxWidth?: number;
		customWidth?: number;
		autoWidth?: boolean;
		flex?: string;
		options: readonly SelectItem<T>[];
		value?: T;
		placeholder?: string;
		maxHeight?: number;
		searchable?: boolean;
		popupAlign?: 'left' | 'right' | 'center';
		customSelectButton?: Snippet;
		itemSnippet: Snippet<[{ item: SelectItem<T>; highlighted: boolean; idx: number }]>;
		children?: Snippet;
		onselect?: (value: T) => void;
		ontoggle?: (isOpen: boolean) => void;
	}
</script>

<script lang="ts" generics="T extends string">
	import OptionsGroup from './OptionsGroup.svelte';
	import SearchItem from './SearchItem.svelte';
	import Textbox from '$lib/Textbox.svelte';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { portal } from '$lib/utils/portal';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { resizeObserver } from '$lib/utils/resizeObserver';
	import { KeyName } from '$lib/utils/hotkeys';
	import { type Snippet } from 'svelte';

	const {
		id,
		label,
		disabled,
		loading,
		wide,
		maxWidth,
		customWidth,
		autoWidth,
		flex,
		options = [],
		value,
		placeholder = 'Select an option...',
		maxHeight,
		searchable,
		popupAlign = 'left',
		customSelectButton,
		itemSnippet,
		children,
		onselect,
		ontoggle
	}: Props = $props();

	let selectWrapperEl: HTMLElement;
	let optionsGroupEl = $state<HTMLElement>();

	let highlightedIndex: number | undefined = $state(undefined);
	let searchValue = $state('');
	const filteredOptions = $derived(
		options.filter((item) => item.label.toLowerCase().includes(searchValue.toLowerCase()))
	);
	let maxHeightState = $state(maxHeight);
	let listOpen = $state(false);
	let inputBoundingRect = $state<DOMRect>();
	let optionsGroupBoundingRect = $state<DOMRect>();

	const maxBottomPadding = 20;

	function setMaxHeight() {
		if (maxHeight) return;
		maxHeightState =
			window.innerHeight - selectWrapperEl.getBoundingClientRect().bottom - maxBottomPadding;
	}

	function openList() {
		setMaxHeight();
		listOpen = true;
		ontoggle?.(true);
	}

	function closeList() {
		listOpen = false;
		ontoggle?.(false);
	}

	function clickOutside(e: MouseEvent) {
		if (e.target === e.currentTarget) closeList();
	}

	function getInputBoundingRect() {
		if (selectWrapperEl) {
			inputBoundingRect = selectWrapperEl.getBoundingClientRect();
		}
		if (optionsGroupEl) {
			optionsGroupBoundingRect = optionsGroupEl.getBoundingClientRect();
		}
	}

	function toggleList() {
		getInputBoundingRect();

		if (listOpen) closeList();
		else openList();
	}

	function handleSelect(item: SelectItem<string>) {
		const value = item.value as T;
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

	function handleKeyDown(e: KeyboardEvent) {
		if (!listOpen) {
			return;
		}
		e.stopPropagation();
		e.preventDefault();

		const { key } = e;

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

	function getTopStyle() {
		if (inputBoundingRect?.top) {
			return `${inputBoundingRect.top + inputBoundingRect.height}px`;
		}
	}

	function getLeftStyle() {
		if (inputBoundingRect?.left && popupAlign === 'left') {
			return `${inputBoundingRect.left}px`;
		}

		if (optionsGroupBoundingRect && inputBoundingRect && popupAlign === 'center') {
			return `${window.innerWidth / 2 - optionsGroupBoundingRect.width / 2}px`;
		}

		if (inputBoundingRect?.left && optionsGroupBoundingRect && popupAlign === 'right') {
			return `${inputBoundingRect.left + inputBoundingRect.width - optionsGroupBoundingRect.width}px`;
		}
	}

	function getPopupWidthStyle() {
		if (customWidth) {
			return '100%';
		}

		if (autoWidth) {
			return 'fit-content';
		}

		return `${inputBoundingRect?.width}px`;
	}
</script>

<div
	class="select-wrapper"
	class:wide
	bind:this={selectWrapperEl}
	style:flex
	style:max-width={maxWidth ? pxToRem(maxWidth) : undefined}
>
	{#if label}
		<label for={id} class="select__label text-13 text-body text-semibold">{label}</label>
	{/if}
	{#if customSelectButton}
		<div
			role="presentation"
			class="select__custom-button"
			onmousedown={toggleList}
			onkeydown={(ev) => handleKeyDown(ev)}
		>
			{@render customSelectButton()}
		</div>
	{:else}
		<Textbox
			{id}
			{placeholder}
			noselect
			readonly
			type="select"
			reversedDirection
			icon="select-chevron"
			value={options.find((item) => item.value === value)?.label}
			disabled={disabled || loading}
			onmousedown={toggleList}
			onkeydown={(ev) => handleKeyDown(ev)}
		/>
	{/if}
	{#if listOpen}
		<div
			role="presentation"
			class="overlay-wrapper"
			onclick={clickOutside}
			use:portal={'body'}
			use:resizeObserver={() => {
				getInputBoundingRect();
				setMaxHeight();
			}}
		>
			<div
				class="options card"
				bind:this={optionsGroupEl}
				style:width={getPopupWidthStyle()}
				style:max-width={customWidth && pxToRem(customWidth)}
				style:top={getTopStyle()}
				style:left={getLeftStyle()}
				style:max-height={maxHeightState && `${maxHeightState}px`}
			>
				<ScrollableContainer whenToShow="scroll">
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
								{@render itemSnippet({ item, highlighted: idx === highlightedIndex, idx })}
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
		height: fit-content;
		width: fit-content;
	}

	.select__label {
		text-align: left;
		color: var(--clr-scale-ntrl-50);
	}

	.select__custom-button {
		display: flex;
		width: fit-content;
	}

	.overlay-wrapper {
		z-index: var(--z-blocker);
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
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
		min-width: 80px;

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

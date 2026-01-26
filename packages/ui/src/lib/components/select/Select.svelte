<script lang="ts" module>
	type T = string;

	export type SelectItem<T extends string = string> = {
		label?: string;
		value?: T;
		selectable?: boolean;
		separator?: boolean; // When true, this item acts as a separator
		[key: string]: any; // Allow additional properties for icons, emojis, etc.
	} & (
		| { separator: true } // Separator items don't need label or value
		| { label: string; value: T } // Regular items require label and value
	);

	type Modifiers = { shift: boolean; ctrl: boolean; alt: boolean; meta: boolean };

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
		minHeight?: number;
		searchable?: boolean;
		popupAlign?: 'left' | 'right' | 'center';
		popupVerticalAlign?: 'top' | 'bottom';
		customSelectButton?: Snippet;
		itemSnippet: Snippet<[{ item: SelectItem<T>; highlighted: boolean; idx: number }]>;
		children?: Snippet;
		icon?: keyof typeof iconsJson;
		autofocus?: boolean;
		onselect?: (value: T, modifiers?: Modifiers) => void;
		ontoggle?: (isOpen: boolean) => void;
	}
</script>

<script lang="ts" generics="T extends string">
	import Textbox from '$components/Textbox.svelte';
	import ScrollableContainer from '$components/scroll/ScrollableContainer.svelte';
	import OptionsGroup from '$components/select/OptionsGroup.svelte';
	import SearchItem from '$components/select/SearchItem.svelte';
	import { KeyName } from '$lib/utils/hotkeys';
	import { portal } from '$lib/utils/portal';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { resizeObserver } from '$lib/utils/resizeObserver';

	import { type Snippet } from 'svelte';
	import type iconsJson from '$lib/data/icons.json';

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
		minHeight,
		searchable,
		popupAlign = 'left',
		popupVerticalAlign = 'bottom',
		customSelectButton,
		itemSnippet,
		children,
		icon,
		autofocus,
		onselect,
		ontoggle
	}: Props = $props();

	let selectWrapperEl: HTMLElement;
	let selectInputEl = $state<HTMLElement>();
	let optionsGroupEl = $state<HTMLElement>();
	let searchItemEl = $state<SearchItem>();
	let selectTriggerEl = $state<HTMLElement | { focus(): void }>();

	let highlightedIndex: number | undefined = $state(undefined);
	let searchValue = $state('');
	const filteredOptions = $derived(
		options.filter(
			(item) =>
				item.separator ||
				(item.label && item.label.toLowerCase().includes(searchValue.toLowerCase()))
		)
	);

	// Group options by separators
	const groupedOptions = $derived.by(() => {
		const groups: SelectItem<T>[][] = [];
		let currentGroup: SelectItem<T>[] = [];

		for (const option of filteredOptions) {
			if (option.separator) {
				if (currentGroup.length > 0) {
					groups.push(currentGroup);
					currentGroup = [];
				}
			} else {
				currentGroup.push(option as SelectItem<T>);
			}
		}

		if (currentGroup.length > 0) {
			groups.push(currentGroup);
		}

		return groups;
	});

	// Flatten grouped options for navigation while preserving order
	const selectableOptions = $derived.by(
		() => filteredOptions.filter((item) => !item.separator) as SelectItem<T>[]
	);

	// Auto-highlight first option when search results change, reset when search is cleared
	$effect(() => {
		if (listOpen && selectableOptions.length > 0 && searchValue.length > 0) {
			highlightedIndex = 0;
		} else if (listOpen && searchValue.length === 0) {
			highlightedIndex = undefined;
		}
	});
	let maxHeightState = $state(maxHeight);
	let listOpen = $state(false);
	let inputBoundingRect = $state<DOMRect>();
	let optionsGroupBoundingRect = $state<DOMRect>();
	let computedVerticalAlign = $state<'top' | 'bottom'>(popupVerticalAlign);

	const maxBottomPadding = 20;

	function setMaxHeight() {
		if (maxHeight) return;
		const rect = selectInputEl?.getBoundingClientRect() || selectWrapperEl.getBoundingClientRect();
		const availableSpaceBelow = window.innerHeight - rect.bottom - maxBottomPadding;
		const availableSpaceAbove = rect.top - maxBottomPadding;

		// Auto-position based on minHeight if provided
		if (minHeight && popupVerticalAlign === 'bottom') {
			if (availableSpaceBelow < minHeight && availableSpaceAbove >= minHeight) {
				computedVerticalAlign = 'top';
				maxHeightState = availableSpaceAbove;
			} else {
				computedVerticalAlign = 'bottom';
				maxHeightState = availableSpaceBelow;
			}
		} else {
			computedVerticalAlign = popupVerticalAlign;
			maxHeightState = availableSpaceBelow;
		}
	}

	function openList() {
		setMaxHeight();
		listOpen = true;
		ontoggle?.(true);

		// Auto-focus search input when dropdown opens and search is available
		if (searchable && options.length > 5) {
			setTimeout(() => {
				searchItemEl?.focus();
			}, 0);
		}
	}

	function closeList(maintainFocus: boolean = false) {
		listOpen = false;
		ontoggle?.(false);

		// Maintain focus on the select trigger if requested
		if (maintainFocus && selectTriggerEl) {
			setTimeout(() => {
				selectTriggerEl?.focus();
			}, 0);
		}
	}

	function clickOutside(e: MouseEvent) {
		if (e.target === e.currentTarget) closeList();
	}

	function getInputBoundingRect() {
		if (selectInputEl) {
			inputBoundingRect = selectInputEl.getBoundingClientRect();
		}
		if (optionsGroupEl) {
			optionsGroupBoundingRect = optionsGroupEl.getBoundingClientRect();
		}
	}

	export function toggleList() {
		getInputBoundingRect();

		if (listOpen) {
			closeList();
		} else if (!disabled) {
			openList();
		}
	}

	function handleSelect(item: SelectItem<string>, event?: MouseEvent | KeyboardEvent) {
		if (item.separator || !item.value) return;
		const value = item.value as T;
		const modifiers = event
			? {
					shift: event.shiftKey,
					ctrl: event.ctrlKey,
					alt: event.altKey,
					meta: event.metaKey
				}
			: undefined;
		onselect?.(value, modifiers);

		// Maintain focus if selection was made via keyboard
		const isKeyboardSelection = event instanceof KeyboardEvent;
		closeList(isKeyboardSelection);
	}

	function handleEnter(event: KeyboardEvent) {
		const option = highlightedIndex !== undefined ? selectableOptions[highlightedIndex] : undefined;
		if (option) {
			handleSelect(option, event);
		} else if (selectableOptions.length > 0) {
			// If no option is highlighted but options exist, select the first one
			handleSelect(selectableOptions[0], event);
		}
	}

	function handleArrowUp() {
		if (selectableOptions.length === 0) return;
		if (highlightedIndex === undefined) {
			highlightedIndex = selectableOptions.length - 1;
		} else {
			highlightedIndex =
				highlightedIndex === 0 ? selectableOptions.length - 1 : highlightedIndex - 1;
		}
	}

	function handleArrowDown() {
		if (selectableOptions.length === 0) return;
		if (highlightedIndex === undefined) {
			highlightedIndex = 0;
		} else {
			highlightedIndex =
				highlightedIndex === selectableOptions.length - 1 ? 0 : highlightedIndex + 1;
		}
	}

	function handleKeyDown(e: KeyboardEvent) {
		const { key } = e;

		if (!listOpen) {
			// When list is closed, handle keys to open it
			if (key === KeyName.Enter || key === KeyName.Down || key === KeyName.Up) {
				e.preventDefault();
				e.stopPropagation();
				openList();
				if (key === KeyName.Up) {
					highlightedIndex = selectableOptions.length - 1;
				} else if (key === KeyName.Down) {
					highlightedIndex = 0;
				}
			}
			return;
		}

		// When list is open, handle navigation
		e.preventDefault();

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
				handleEnter(e);
				break;
		}
	}

	function getTopStyle() {
		const gap = 4; // Gap between input and dropdown in pixels

		if (computedVerticalAlign === 'bottom') {
			if (inputBoundingRect?.top) {
				return `${inputBoundingRect.top + inputBoundingRect.height + gap}px`;
			}
		}

		if (computedVerticalAlign === 'top') {
			if (inputBoundingRect?.top && optionsGroupBoundingRect) {
				return `${inputBoundingRect.top - optionsGroupBoundingRect.height - gap}px`;
			}
		}
	}

	function getLeftStyle() {
		if (inputBoundingRect?.left && popupAlign === 'left') {
			return `${inputBoundingRect.left}px`;
		}

		if (optionsGroupBoundingRect && inputBoundingRect && popupAlign === 'center') {
			return `${inputBoundingRect.left + inputBoundingRect.width / 2 - optionsGroupBoundingRect.width / 2}px`;
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
	style:max-width={maxWidth ? `${pxToRem(maxWidth)}rem` : 'none'}
>
	{#if label}
		<label for={id} class="select__label text-13 text-body text-semibold">{label}</label>
	{/if}
	{#if customSelectButton}
		<div
			bind:this={selectInputEl}
			role="presentation"
			class="select__custom-button"
			onclick={toggleList}
			onkeydown={(ev: KeyboardEvent) => handleKeyDown(ev)}
		>
			<div bind:this={selectTriggerEl}>
				{@render customSelectButton()}
			</div>
		</div>
	{:else}
		<div bind:this={selectInputEl}>
			<Textbox
				bind:this={selectTriggerEl}
				{id}
				{placeholder}
				readonly
				type="select"
				iconLeft={icon}
				iconRight="select-chevron"
				value={options.find((item) => item.value === value)?.label}
				disabled={disabled || loading}
				{autofocus}
				onmousedown={toggleList}
				onkeydown={(ev: KeyboardEvent) => handleKeyDown(ev)}
			/>
		</div>
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
				style:max-width={customWidth && `${pxToRem(customWidth)}rem`}
				style:top={getTopStyle()}
				style:left={getLeftStyle()}
				style:max-height={maxHeightState && `${maxHeightState}px`}
				role="listbox"
				tabindex="-1"
				onkeydown={(ev: KeyboardEvent) => handleKeyDown(ev)}
			>
				<ScrollableContainer whenToShow="scroll">
					{#if searchable && options.length > 5}
						<SearchItem bind:this={searchItemEl} bind:searchValue />
					{/if}
					{#if groupedOptions.length === 0}
						<OptionsGroup>
							<div class="text-13 text-semibold option nothing-found">
								<span class=""> Nothing found ¯\_(ツ)_/¯ </span>
							</div>
						</OptionsGroup>
					{:else}
						{#each groupedOptions as group}
							<OptionsGroup>
								{#each group as item}
									{@const selectableIdx = selectableOptions.findIndex(
										(opt) => opt.value === item.value && item.value !== undefined
									)}
									<div
										class="option"
										tabindex="-1"
										role="none"
										onclick={(event) => handleSelect(item, event)}
									>
										{@render itemSnippet({
											item,
											highlighted: selectableIdx === highlightedIndex,
											idx: selectableIdx
										})}
									</div>
								{/each}
							</OptionsGroup>
						{/each}
					{/if}

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
		display: flex;
		position: relative;
		flex-direction: column;
		height: fit-content;
		gap: 6px;
	}

	.select__label {
		color: var(--clr-text-2);
		text-align: left;
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
		z-index: var(--z-floating);
		position: absolute;
		min-width: 80px;
		overflow: hidden;
		transform-origin: top;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-s);

		animation: fadeIn 0.16s ease-out forwards;
		user-select: none;
	}

	@keyframes fadeIn {
		0% {
			transform: translateY(-6px);
			opacity: 0;
		}
		40% {
			opacity: 1;
		}
		100% {
			transform: translateY(0);
			opacity: 1;
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

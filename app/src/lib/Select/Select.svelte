<script lang="ts" context="module">
	export type SelectItem = {
		label: string;
		value: string;
		selectable?: boolean;
	};
</script>

<script lang="ts">
	import OptionsGroup from './OptionsGroup.svelte';
	import SearchItem from './SearchItem.svelte';
	import ScrollableContainer from '../shared/ScrollableContainer.svelte';
	import TextBox from '../shared/TextBox.svelte';
	import { portal } from '$lib/utils/portal';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { resizeObserver } from '$lib/utils/resizeObserver';
	import type { Snippet } from 'svelte';

	interface SelectProps {
		id?: string;
		label?: string;
		disabled?: boolean;
		loading?: boolean;
		wide?: boolean;
		options: SelectItem[];
		value?: any;
		placeholder?: string;
		maxHeight?: number;
		searchable?: boolean;
		itemSnippet: Snippet<[item: any, selected?: boolean]>;
		children?: Snippet;
		onselect?: (value: string) => void;
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

	let filteredOptions = $state(options);
	let maxHeightState = $state(maxHeight);
	let listOpen = $state(false);
	let inputBoundingRect = $state<DOMRect>();
	let optionsEl = $state<HTMLDivElement>();

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

	function handleSelect(item: { label: string; value: string }) {
		const value = item.value;
		onselect?.(value);
		closeList();
	}
</script>

<div class="select-wrapper" class:wide bind:this={selectWrapperEl}>
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
		value={options.find((item) => item.value === value)?.label || 'Select an option...'}
		disabled={disabled || loading}
		on:mousedown={toggleList}
	/>
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
				bind:this={optionsEl}
				class="options card"
				style:width="{inputBoundingRect?.width}px"
				style:top={inputBoundingRect?.top
					? pxToRem(inputBoundingRect.top + inputBoundingRect.height)
					: undefined}
				style:left={inputBoundingRect?.left ? pxToRem(inputBoundingRect.left) : undefined}
				style:max-height={maxHeightState && pxToRem(maxHeightState)}
			>
				<ScrollableContainer initiallyVisible>
					{#if searchable && options.length > 5}
						<SearchItem
							items={options}
							onSort={(filtered) => {
								filteredOptions = filtered;
							}}
						/>
					{/if}
					<OptionsGroup>
						{#if filteredOptions.length === 0}
							<div class="text-base-13 text-semibold option nothing-found">
								<span class=""> Nothing found ¯\_(ツ)_/¯ </span>
							</div>
						{/if}
						{#each filteredOptions as item}
							<div
								class="option"
								class:selected={item === value}
								tabindex="-1"
								role="none"
								onmousedown={() => handleSelect(item)}
							>
								{@render itemSnippet(item)}
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
		50% {
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

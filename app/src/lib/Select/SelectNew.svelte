<script lang="ts">
	import OptionsGroup from './OptionsGroup.svelte';
	import ScrollableContainer from '../shared/ScrollableContainer.svelte';
	import TextBox from '../shared/TextBox.svelte';
	import { portal } from '$lib/utils/portal';
	import { pxToRem } from '$lib/utils/pxToRem';
	import type { Snippet } from 'svelte';

	interface SelectProps {
		id?: string;
		label?: string;
		disabled?: boolean;
		loading?: boolean;
		wide?: boolean;
		options: {
			label: string;
			value: string;
			selectable?: boolean;
		}[];
		value?: any;
		placeholder?: string;
		maxHeight?: number;
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
		itemSnippet,
		children,
		onselect
	}: SelectProps = $props();

	let selectWrapperEl: HTMLElement;
	let maxHeightState = $state(maxHeight);
	let listOpen = $state(false);
	let inputBoundingRect = $state<DOMRect>();
	let optionsEl = $state<HTMLDivElement>();

	const maxBottomPadding = 20;

	function setMaxHeight() {
		if (maxHeight) return;
		maxHeightState =
			window.innerHeight - selectWrapperEl.getBoundingClientRect().bottom - maxBottomPadding;

		console.log('maxHeightState', maxHeightState);
	}

	function openList() {
		setMaxHeight();
		listOpen = true;
	}

	function closeList() {
		listOpen = false;
	}

	function toggleList(e: MouseEvent) {
		const target = e.target as HTMLElement;
		if (target) {
			inputBoundingRect = target.getBoundingClientRect();
		}

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
		on:mousedown={(ev) => toggleList(ev)}
	/>
	{#if listOpen}
		<div
			role="presentation"
			class="scroll-blocker"
			onclick={(e: MouseEvent) => {
				if (e.target === e.currentTarget) closeList();
			}}
			use:portal={'body'}
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
					<OptionsGroup>
						{#each options as item}
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

	.scroll-blocker {
		z-index: var(--z-blocker);
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background-color: rgba(0, 0, 0, 0.1);
	}

	.options {
		position: absolute;
		/* right: 0; */
		/* top: 100%;
		width: 100%; */
		z-index: var(--z-floating);
		margin-top: 4px;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-s);
		overflow: hidden;
	}

	.wide {
		width: 100%;
	}
</style>

<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { createEventDispatcher, getContext, onMount } from 'svelte';
	import type iconsJson from '$lib/icons/icons.json';
	import type { SegmentContext } from './segment';

	export let id: string;
	export let disabled = false;
	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let label: string | undefined = undefined;

	let ref: HTMLButtonElement | undefined;
	const dispatcher = createEventDispatcher<{ select: string }>();

	const context = getContext<SegmentContext>('SegmentedControl');
	const index = context.setIndex();
	const focusedSegmentIndex = context.focusedSegmentIndex;
	const selectedSegmentIndex = context.selectedSegmentIndex;
	const length = context.length;

	$: isFocused = $focusedSegmentIndex === index;
	$: if (isFocused) {
		ref?.focus();
	}
	$: isSelected = $selectedSegmentIndex === index;

	onMount(() => {
		context.addSegment({ id, index, disabled });
	});
</script>

<button
	bind:this={ref}
	class="btn"
	class:left={index == 0}
	class:right={index == $length - 1}
	role="tab"
	aria-selected={isSelected}
	aria-disabled={disabled}
	tabindex={isSelected ? 0 : -1}
	{...$$restProps}
	on:mousedown|preventDefault={() => {
		if (index !== $selectedSegmentIndex && !disabled) {
			context.setSelected(index);
			dispatcher('select', id);
		}
	}}
	on:keydown={({ key }) => {
		if (key === 'ArrowRight') {
			context.setSelected(index + 1);
		} else if (key === 'ArrowLeft') {
			context.setSelected(index - 1);
		}
	}}
>
	{#if label}
		<span class="label text-base-12">
			{label}
		</span>
	{/if}
	{#if icon}
		<div class="icon">
			<Icon name={icon} />
		</div>
	{/if}
</button>

<style lang="postcss">
	.btn {
		display: inline-flex;
		flex-grow: 1;
		flex-basis: 0;
		align-items: center;
		justify-content: center;
		gap: var(--size-4);

		height: var(--size-control-m);
		background-color: var(--clr-theme-container-pale);
		padding: var(--size-4) var(--size-8);

		border-top-width: 1px;
		border-bottom-width: 1px;
		border-color: var(--clr-theme-container-outline-light);

		transition: background var(--transition-fast);

		cursor: pointer;

		&:hover {
			background-color: color-mix(
				in srgb,
				var(--clr-theme-container-pale),
				var(--darken-tint-light)
			);
		}

		&[aria-selected='true'] {
			background-color: var(--clr-theme-container-light);
			padding: var(--size-4) var(--size-8);
			border-right-width: 1px;
			border-left-width: 1px;

			cursor: default;

			& > .label {
				color: var(--clr-theme-scale-ntrl-0);
				cursor: default;
			}
			& > .icon {
				color: var(--clr-theme-scale-ntrl-30);
				cursor: default;
			}
			&.left {
				border-right-width: 1px;
			}
			&.right {
				border-left-width: 1px;
			}
		}
		&.left {
			border-left-width: 1px;
			border-top-left-radius: var(--radius-m);
			border-bottom-left-radius: var(--radius-m);
		}
		&.right {
			border-right-width: 1px;
			border-top-right-radius: var(--radius-m);
			border-bottom-right-radius: var(--radius-m);
		}
	}

	.icon {
		display: flex;
		justify-content: center;
		align-items: center;
		color: var(--clr-theme-scale-ntrl-50);
		cursor: pointer;
	}

	.label {
		color: var(--clr-theme-scale-ntrl-40);
		cursor: pointer;
	}
</style>

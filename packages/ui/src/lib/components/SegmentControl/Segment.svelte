<script lang="ts">
	import { createEventDispatcher, getContext, onMount } from 'svelte';
	import type { SegmentContext } from './segment';
	import type iconsJson from '$lib/icons/icons.json';
	import Icon from '$lib/icons/Icon.svelte';

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
	on:click|preventDefault={() => {
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
		gap: var(--space-4);

		height: var(--size-btn-m);
		background-color: var(--clr-theme-container-pale);
		padding: var(--space-4) var(--space-8);

		border-width: 1px;
		border-top-color: var(--clr-theme-container-outline-light);
		border-bottom-color: var(--clr-theme-container-outline-light);
		border-left-color: transparent;
		border-right-color: transparent;

		transition: background var(--transition-fast);

		&:hover {
			background-color: var(--clr-theme-container-mid);
		}

		&[aria-selected='true'] {
			background-color: var(--clr-theme-container-light);
			border-left-color: var(--clr-theme-container-outline-light);
			border-right-color: var(--clr-theme-container-outline-light);

			& > .label {
				color: var(--clr-theme-scale-ntrl-0);
			}
			& > .icon {
				color: var(--clr-theme-scale-ntrl-0);
			}
			&.left {
				border-right-color: var(--clr-theme-container-outline-light);
			}
			&.right {
				border-left-color: var(--clr-theme-container-outline-light);
			}
		}
		&.left {
			border-top-left-radius: var(--radius-m);
			border-left-color: var(--clr-theme-container-outline-light);
			border-bottom-left-radius: var(--radius-m);
		}
		&.right {
			border-right-color: var(--clr-theme-container-outline-light);
			border-top-right-radius: var(--radius-m);
			border-bottom-right-radius: var(--radius-m);
		}
	}

	.icon {
		color: var(--clr-theme-scale-ntrl-50);
	}

	.label {
		color: var(--clr-theme-scale-ntrl-40);
	}
</style>

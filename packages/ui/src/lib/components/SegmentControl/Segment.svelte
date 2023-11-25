<script lang="ts">
	import { createEventDispatcher, getContext, onMount } from 'svelte';
	import type { SegmentContext } from './segment';
	import type iconsJson from '$lib/icons/icons.json';
	import Icon from '$lib/icons/Icon.svelte';

	export let id: string;
	export let disabled = false;
	export let icon: keyof typeof iconsJson | undefined = undefined;

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
	<span class="text-base-12">
		<slot />
	</span>
	{#if icon}
		<Icon name={icon} />
	{/if}
</button>

<style lang="postcss">
	.btn {
		display: inline-flex;
		flex-grow: 1;
		flex-basis: 0;
		align-items: center;
		gap: var(--space-4);
		justify-content: center;
		background-color: var(--clr-theme-container-pale);
		border-color: var(--clr-theme-container-outline-light);
		padding-top: var(--space-4);
		padding-bottom: var(--space-4);
		padding-left: var(--space-8);
		padding-right: var(--space-8);
		border-top-width: 1px;
		border-bottom-width: 1px;
		color: var(--clr-theme-scale-ntrl-40);

		&[aria-selected='true'] {
			background-color: var(--clr-theme-container-light);
			border-left-width: 1px;
			color: var(--clr-theme-scale-ntrl-10);
			&.left {
				border-right-width: 1px;
			}
			&.right {
				border-left-width: 1px;
			}
			border-right-width: 1px;
		}
		&.left {
			border-top-left-radius: var(--radius-m);
			border-left-width: 1px;
			border-bottom-left-radius: var(--radius-m);
		}
		&.right {
			border-right-width: 1px;
			border-top-right-radius: var(--radius-m);
			border-bottom-right-radius: var(--radius-m);
		}
	}
</style>

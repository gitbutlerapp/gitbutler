<script lang="ts">
	import Icon from '$lib/shared/Icon.svelte';
	import { createEventDispatcher, getContext, onMount } from 'svelte';
	import type iconsJson from '$lib/icons/icons.json';
	import type { SegmentContext } from './segment';

	export let id: string;
	export let disabled = false;
	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let label: string | undefined = undefined;
	export let size: 'small' | 'medium' = 'medium';

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
	class="segment-btn segment-{size}"
	class:left={index === 0}
	class:right={index === $length - 1}
	role="tab"
	tabindex={isSelected ? -1 : 0}
	aria-selected={isSelected}
	aria-disabled={disabled}
	{...$$restProps}
	on:mousedown|preventDefault={() => {
		if (index !== $selectedSegmentIndex && !disabled) {
			context.setSelected(index);
			dispatcher('select', id);
		}
	}}
	on:keydown={({ key }) => {
		if (key === 'Enter' || key === ' ') {
			if (index !== $selectedSegmentIndex && !disabled) {
				context.setSelected(index);
				dispatcher('select', id);
			}
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
	.segment-btn {
		cursor: pointer;
		display: inline-flex;
		flex-grow: 1;
		flex-basis: 0;
		align-items: center;
		justify-content: center;
		gap: 4px;

		height: var(--size-button);

		border-top-width: 1px;
		border-bottom-width: 1px;
		border-left-width: 1px;

		border-color: var(--clr-border-2);

		transition: background var(--transition-fast);

		&[aria-selected='true'] {
			background-color: var(--clr-bg-2);

			cursor: default;

			& > .label,
			& > .icon {
				color: var(--clr-scale-ntrl-50);
				cursor: default;
			}

			&:focus {
				outline: none;
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
		color: var(--clr-scale-ntrl-30);
	}

	.label {
		color: var(--clr-scale-ntrl-30);
	}

	/* MODIFIERS */
	.segment-small {
		height: var(--size-tag);
		padding: 2px 4px;
	}

	.segment-medium {
		height: var(--size-button);
		padding: 4px 8px;
	}
</style>

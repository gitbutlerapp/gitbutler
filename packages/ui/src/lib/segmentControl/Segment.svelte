<script lang="ts">
	import { getContext, onMount } from 'svelte';
	import type { SegmentContext } from './segmentTypes';
	import type { Snippet } from 'svelte';

	interface SegmentProps {
		id: string;
		onselect?: (id: string) => void;
		disabled?: boolean;
		unfocusable?: boolean;
		children: Snippet;
	}

	const { id, onselect, children, disabled = false, unfocusable = false }: SegmentProps = $props();

	const context = getContext<SegmentContext>('SegmentControl');
	const index = context.setIndex();
	const selectedSegmentIndex = context.selectedSegmentIndex;

	let elRef = $state<HTMLButtonElement>();
	let isFocused = $state(false);
	let isSelected = $state(false);

	$effect(() => {
		elRef && isFocused && elRef.focus();
	});

	$effect(() => {
		isSelected = $selectedSegmentIndex === index;
	});

	onMount(() => {
		context.addSegment({ index });
	});
</script>

<button
	bind:this={elRef}
	{id}
	class="segment-control-item"
	role="tab"
	{disabled}
	tabindex={isSelected || unfocusable ? -1 : 0}
	aria-selected={isSelected}
	onclick={() => {
		if (index !== $selectedSegmentIndex) {
			context.setSelected({
				index,
				id
			});
			onselect && onselect(id);
		}
	}}
	onkeydown={({ key }) => {
		if (key === 'Enter' || key === ' ') {
			if (index !== $selectedSegmentIndex) {
				context.setSelected({
					index,
					id
				});
				onselect && onselect(id);
			}
		}
	}}
>
	<span class="text-12 segment-control-item__label">
		{@render children()}
	</span>
</button>

<!-- <style lang="postcss">
	.segment {
		cursor: pointer;
		display: inline-flex;
		flex-grow: 1;
		flex-basis: 0;
		align-items: center;
		justify-content: center;
		user-select: none;
		padding: 0 8px;
		gap: 4px;

		border-top-width: 1px;
		border-bottom-width: 1px;
		border-left-width: 1px;

		color: var(--clr-text-1);
		border-color: var(--clr-border-2);
		background-color: var(--clr-bg-1);
		height: var(--size-button);

		transition: background var(--transition-fast);

		&:first-of-type {
			border-top-left-radius: var(--radius-m);
			border-bottom-left-radius: var(--radius-m);
		}

		&:last-of-type {
			border-top-right-radius: var(--radius-m);
			border-bottom-right-radius: var(--radius-m);
			border-right-width: 1px;
		}

		&:not([aria-selected='true']):hover {
			background-color: var(--clr-bg-1-muted);
		}

		&[aria-selected='true'] {
			background-color: var(--clr-bg-2);
			color: var(--clr-text-2);
		}

		&:disabled {
			cursor: default;
			opacity: 0.5;
		}
	}

	.label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		transition: color var(--transition-fast);
	}
</style> -->

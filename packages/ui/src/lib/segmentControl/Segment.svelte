<script lang="ts">
	import { getContext, onMount } from 'svelte';
	import type { SegmentContext } from './segmentTypes';
	import type { Snippet } from 'svelte';

	interface SegmentProps {
		id: string;
		onselect?: (id: string) => void;
		disabled?: boolean;
		children: Snippet;
	}

	const { id, onselect, children, disabled }: SegmentProps = $props();

	const context = getContext<SegmentContext>('SegmentControl');
	const index = context.setIndex();
	const selectedSegmentIndex = context.selectedSegmentIndex;

	let elRef = $state<HTMLButtonElement>();
	let isFocused = $state(false);
	let isSelected = $state(false);

	$effect(() => {
		if (elRef && isFocused) {
			elRef.focus();
		}
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
	tabindex={isSelected || disabled ? -1 : 0}
	aria-selected={isSelected}
	onclick={() => {
		if (index !== $selectedSegmentIndex) {
			context.setSelected({
				index,
				id
			});
			if (onselect) {
				onselect(id);
			}
		}
	}}
	onkeydown={({ key }) => {
		if (key === 'Enter' || key === ' ') {
			if (index !== $selectedSegmentIndex) {
				context.setSelected({
					index,
					id
				});
				if (onselect) {
					onselect(id);
				}
			}
		}
	}}
>
	<span class="text-12 segment-control-item__label">
		{@render children()}
	</span>
</button>

<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import { getContext, onMount } from 'svelte';
	import type { SegmentContext } from '$lib/segmentControl/segmentTypes';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	interface SegmentProps {
		id: string;
		onselect?: (id: string) => void;
		disabled?: boolean;
		children?: Snippet;
		icon?: keyof typeof iconsJson;
	}

	const { id, onselect, children, disabled, icon }: SegmentProps = $props();

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
	type="button"
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
	{#if children}
		<span class="text-12 text-semibold segment-control-item__label">
			{@render children()}
		</span>
	{/if}
	{#if icon}
		<span class="segment-control-item__icon" aria-hidden="true">
			<Icon name={icon} />
		</span>
	{/if}
</button>

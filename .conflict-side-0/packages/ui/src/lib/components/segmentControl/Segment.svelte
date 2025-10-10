<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import Tooltip from '$components/Tooltip.svelte';
	import { getContext, onMount } from 'svelte';
	import type { SegmentContext } from '$components/segmentControl/segmentTypes';
	import type iconsJson from '$lib/data/icons.json';
	import type { Snippet } from 'svelte';

	interface SegmentProps {
		testId?: string;
		id: string;
		onselect?: (id: string) => void;
		disabled?: boolean;
		children?: Snippet;
		tooltip?: string;
		tooltipPosition?: 'top' | 'bottom';
		icon?: keyof typeof iconsJson;
	}

	const { id, onselect, children, disabled, icon, tooltip, tooltipPosition, testId }: SegmentProps =
		$props();

	const context = getContext<SegmentContext>('SegmentControl');
	const index = context.setIndex();
	const selectedSegmentIndex = context.selectedSegmentIndex;

	let elRef = $state<HTMLButtonElement>();
	let isFocused = $state(false);
	const isSelected = $derived(index === $selectedSegmentIndex);

	$effect(() => {
		if (elRef && isFocused) {
			elRef.focus();
		}
	});

	onMount(() => {
		context.addSegment({ index });
	});
</script>

<Tooltip text={tooltip} position={tooltipPosition}>
	<button
		data-testid={testId}
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
</Tooltip>

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
		disabled?: boolean;
		children?: Snippet;
		tooltip?: string;
		tooltipPosition?: 'top' | 'bottom';
		icon?: keyof typeof iconsJson;
	}

	const { id, children, disabled, icon, tooltip, tooltipPosition, testId }: SegmentProps = $props();

	const context = getContext<SegmentContext>('SegmentControl');
	const selectedSegmentId = context.selectedSegmentId;

	let elRef = $state<HTMLButtonElement>();
	let isFocused = $state(false);
	const isSelected = $derived($selectedSegmentId === id);

	$effect(() => {
		if (elRef && isFocused) {
			elRef.focus();
		}
	});

	onMount(() => {
		context.registerSegment(id);
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
			if (!isSelected) {
				context.selectSegment(id);
			}
		}}
		onkeydown={({ key }) => {
			if (key === 'Enter' || key === ' ') {
				if (!isSelected) {
					context.selectSegment(id);
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

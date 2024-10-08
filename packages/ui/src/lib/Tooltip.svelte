<script lang="ts" module>
	export type TooltipPosition = 'top' | 'bottom';
	export type TooltipAlign = 'start' | 'center' | 'end';
</script>

<script lang="ts">
	import { portal } from '$lib/utils/portal';
	import { setPosition } from '$lib/utils/tooltipPosition';
	import { flyScale } from '$lib/utils/transitions';
	import { type Snippet } from 'svelte';

	interface Props {
		text?: string;
		delay?: number;
		align?: TooltipAlign;
		position?: TooltipPosition;
		children: Snippet;
	}

	const { text, delay = 700, align, position, children }: Props = $props();

	let targetEl: HTMLElement | undefined = $state();
	let tooltipEl: HTMLElement | undefined = $state();

	let show = $state(false);
	let timeoutId: undefined | ReturnType<typeof setTimeout> = $state();

	const isTextEmpty = $derived(!text || text === '');

	function handleMouseEnter() {
		timeoutId = setTimeout(() => {
			show = true;
			// console.log('showing tooltip');
		}, delay); // 500ms delay before showing the tooltip
	}

	function handleMouseLeave() {
		clearTimeout(timeoutId);
		show = false;
	}
</script>

{#if isTextEmpty}
	{@render children()}
{:else}
	<span
		bind:this={targetEl}
		class="tooltip-wrap"
		role="tooltip"
		onmouseenter={handleMouseEnter}
		onmouseleave={handleMouseLeave}
	>
		{#if children}
			{@render children()}
		{/if}

		{#if show}
			<div
				bind:this={tooltipEl}
				use:setPosition={{ targetEl, position, align }}
				use:portal={'body'}
				class="tooltip-container text-11 text-body"
				transition:flyScale={{
					position: position
				}}
			>
				<span>{text}</span>
			</div>
		{/if}
	</span>
{/if}

<style lang="postcss">
	.tooltip-wrap {
		position: relative;
		display: contents;
	}

	.tooltip-container {
		white-space: pre-line;
		display: flex;
		justify-content: center;
		flex-direction: column;
		position: fixed;
		pointer-events: none;
		background-color: var(--clr-tooltip-bg);
		border: 1px solid var(--clr-tooltip-border);
		border-radius: var(--radius-m);
		color: var(--clr-core-ntrl-80);
		display: inline-block;
		width: fit-content;
		max-width: 240px;
		padding: 4px 8px;
		z-index: var(--z-blocker);
		text-align: left;
		box-shadow: var(--fx-shadow-s);
	}
</style>

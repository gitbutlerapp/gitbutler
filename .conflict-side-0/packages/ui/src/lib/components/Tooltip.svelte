<script lang="ts" module>
	export type TooltipPosition = 'top' | 'bottom';
	export type TooltipAlign = 'start' | 'center' | 'end';
</script>

<script lang="ts">
	import { portal } from '$lib/utils/portal';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { tooltip } from '$lib/utils/tooltipPosition';
	import { flyScale } from '$lib/utils/transitions';
	import { type Snippet } from 'svelte';

	interface Props {
		text?: string;
		delay?: number;
		disabled?: boolean;
		align?: TooltipAlign;
		position?: TooltipPosition;
		overrideYScroll?: number;
		maxWidth?: number;
		hotkey?: string;
		children: Snippet;
	}

	const {
		text,
		delay = 700,
		disabled,
		align,
		position: requestedPosition = 'bottom',
		overrideYScroll,
		maxWidth = 240,
		hotkey,
		children
	}: Props = $props();

	const TOOLTIP_VIEWPORT_EDGE_MARGIN = 100; // px
	let targetEl: HTMLElement | undefined = $state();
	let position = $state(requestedPosition);
	let show = $state(false);
	let timeoutId: undefined | ReturnType<typeof setTimeout> = $state();

	const isTextEmpty = $derived(!text || text === '');

	$effect(() => {
		if (targetEl && window.visualViewport) {
			// Use child of tooltip wrapper; since tooltip wrapper is 'display:contents'
			// which results in boundingClientRect values all being 0. Plus we care
			// about the child button, icon, etc. anyway
			const { top, bottom } = targetEl.children[0].getBoundingClientRect();

			// Force tooltip to top if within MARGIN of bottom of viewport
			if (window.visualViewport.height - bottom < TOOLTIP_VIEWPORT_EDGE_MARGIN) {
				position = 'top';
			}

			// Force tooltip to bottom if within MARGIN of top of viewport
			if (top < TOOLTIP_VIEWPORT_EDGE_MARGIN) {
				position = 'bottom';
			}
		}
	});

	function handleMouseEnter() {
		if (disabled) return;
		timeoutId = setTimeout(() => {
			show = true;
		}, delay);
	}

	function handleMouseLeave() {
		clearTimeout(timeoutId);
		show = false;
	}

	function handleClick(e: MouseEvent) {
		// Need to prevent interference with context menu and modals
		if ((e.target as HTMLElement)?.dataset.clickable === 'true') {
			e.preventDefault();
			handleMouseLeave();
		}
	}
</script>

{#if isTextEmpty}
	{@render children()}
{:else}
	<span
		bind:this={targetEl}
		class="tooltip-wrap"
		role="presentation"
		onmouseenter={handleMouseEnter}
		onmouseleave={handleMouseLeave}
		onmousedown={handleClick}
	>
		{#if children}
			{@render children()}
		{/if}

		{#if show}
			<div
				use:tooltip={{ targetEl, position, align, overrideYScroll }}
				use:portal={'body'}
				class="tooltip-container text-11 text-body"
				style:max-width="{pxToRem(maxWidth)}rem"
				transition:flyScale={{
					position: position
				}}
			>
				<span>{text}</span>

				{#if hotkey}
					<span class="tooltip-hotkey">{hotkey}</span>
				{/if}
			</div>
		{/if}
	</span>
{/if}

<style lang="postcss">
	.tooltip-wrap {
		display: contents;
		position: relative;
	}

	.tooltip-container {
		display: flex;
		display: inline-block;
		z-index: var(--z-blocker);
		position: fixed;
		flex-direction: column;
		justify-content: center;
		width: fit-content;
		padding: 4px 8px;
		border: 1px solid var(--clr-tooltip-border);
		border-radius: var(--radius-m);
		background-color: var(--clr-tooltip-bg);
		box-shadow: var(--fx-shadow-s);
		color: var(--clr-core-ntrl-80);
		text-align: left;
		white-space: pre-line;
		word-break: break-word;
		pointer-events: none;
	}

	.tooltip-hotkey {
		margin-left: 3px;
		opacity: 0.6;
	}
</style>

<script lang="ts" module>
	export type TooltipWrapperPosition = 'top' | 'bottom';
	export type TooltipWrapperAlign = 'start' | 'center' | 'end';
</script>

<script lang="ts">
	import { portal } from '$lib/utils/portal';
	import { flyScale } from '$lib/utils/transitions';
	import type { Snippet } from 'svelte';

	const DEFAULT_GAP = 4;
	const DEFAULT_DELAY = 700;

	interface Props {
		delay?: number;
		animationDuration?: number;
		align?: TooltipWrapperAlign;
		position?: TooltipWrapperPosition;
		gap?: number;
		disable?: boolean;
		forceShow?: boolean;
		onShow?: () => void;
		children: Snippet;
		tooltip: Snippet;
	}

	let {
		disable = false,
		gap = DEFAULT_GAP,
		delay = DEFAULT_DELAY,
		animationDuration,
		position,
		align,
		forceShow,
		onShow,
		children,
		tooltip
	}: Props = $props();

	let targetEl: HTMLElement | undefined = $state();
	let tooltipEl: HTMLElement | undefined = $state();

	let timeoutId: undefined | ReturnType<typeof setTimeout> = $state();
	let show = $state(false);

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

	function isNoSpaceOnRight() {
		if (!targetEl || !tooltipEl) return false;

		const tooltipRect = tooltipEl.getBoundingClientRect();
		const targetChild = targetEl.children[0];
		const targetRect = targetChild.getBoundingClientRect();

		return targetRect.left + tooltipRect.width / 2 > window.innerWidth;
	}

	function isNoSpaceOnLeft() {
		if (!targetEl || !tooltipEl) return false;

		const tooltipRect = tooltipEl.getBoundingClientRect();
		const targetChild = targetEl.children[0];
		const targetRect = targetChild.getBoundingClientRect();

		return targetRect.left - tooltipRect.width / 2 < 0;
	}

	function adjustPosition() {
		if (!targetEl || !tooltipEl) return;

		const tooltipRect = tooltipEl.getBoundingClientRect();
		// get first child of targetEl
		const targetChild = targetEl.children[0];
		const targetRect = targetChild.getBoundingClientRect();

		let top = 0;
		let left = 0;
		let transformOriginTop = 'center';
		let transformOriginLeft = 'center';

		function alignLeft() {
			left = targetRect.left + window.scrollX;
			transformOriginLeft = 'left';
		}

		function alignRight() {
			left = targetRect.right - tooltipRect.width + window.scrollX;
			transformOriginLeft = 'right';
		}

		function alignCenter() {
			left = targetRect.left + targetRect.width / 2 - tooltipRect.width / 2 + window.scrollX;
			transformOriginLeft = 'center';
		}

		function positionTop() {
			top = targetRect.top - tooltipRect.height + window.scrollY - gap;
			transformOriginTop = 'bottom';
		}

		function positionBottom() {
			top = targetRect.bottom + window.scrollY + gap;
			transformOriginTop = 'top';
		}

		// Vertical position
		if (position) {
			if (position === 'bottom') {
				positionBottom();
			} else if (position === 'top') {
				positionTop();
			}
		} else {
			positionBottom();
		}

		// Auto check horizontal position
		if (align) {
			if (align === 'start') {
				alignLeft();
			} else if (align === 'end') {
				alignRight();
			} else if (align === 'center') {
				alignCenter();
			}
		} else {
			if (isNoSpaceOnLeft()) {
				alignLeft();
			} else if (isNoSpaceOnRight()) {
				alignRight();
			} else {
				alignCenter();
			}
		}

		tooltipEl.style.top = `${top}px`;
		tooltipEl.style.left = `${left}px`;
		tooltipEl.style.transformOrigin = `${transformOriginTop} ${transformOriginLeft}`;
	}

	$effect(() => {
		if (tooltipEl) {
			adjustPosition();
		}
	});

	$effect(() => {
		if (show || forceShow) {
			onShow?.();
		}
	});
</script>

{#if disable}
	{#if children}
		{@render children()}
	{/if}
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

		{#if show || forceShow}
			<div
				bind:this={tooltipEl}
				use:portal={'body'}
				class="tooltip-container text-11 text-body"
				transition:flyScale={{
					position: position,
					duration: animationDuration
				}}
			>
				{#if tooltip}
					{@render tooltip()}
				{/if}
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
		position: fixed;
		z-index: var(--z-blocker);
	}
</style>

<script lang="ts" module>
	export type TooltipPosition = 'top' | 'bottom';
	export type TooltipAlign = 'start' | 'center' | 'end';
</script>

<script lang="ts">
	import { portal } from '$lib/utils/portal';
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
		const gap = 4;

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

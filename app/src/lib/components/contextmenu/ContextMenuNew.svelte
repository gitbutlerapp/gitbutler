<script lang="ts" context="module">
	export type ContextMenuActions = {
		open: () => void;
		close: () => void;
	};
</script>

<script lang="ts">
	import { portal } from '$lib/utils/portal';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { resizeObserver } from '$lib/utils/resizeObserver';
	import { type Snippet } from 'svelte';
	import { onMount } from 'svelte';

	interface Props {
		openByTarget?: HTMLElement;
		verticalAlign?: 'top' | 'bottom';
		horizontalAlign?: 'left' | 'right';
		children: Snippet;
	}

	const {
		openByTarget,
		verticalAlign = 'bottom',
		horizontalAlign = 'right',
		children
	}: Props = $props();

	let contextMenuHeight = $state(0);
	let contextMenuWidth = $state(0);
	let isVisibile = $state(false);
	let targetBoundingRect = $state<DOMRect>();

	let menuMargin = 4;

	export function close() {
		isVisibile = false;
	}

	export function open() {
		isVisibile = true;
	}

	function getTargetBoundingRect() {
		if (openByTarget) {
			targetBoundingRect = openByTarget.getBoundingClientRect();
		}
	}

	function handleTargetClick(e: MouseEvent) {
		e.preventDefault();
		getTargetBoundingRect();
		open();
	}

	function clickOutside(e: MouseEvent) {
		if (e.target === e.currentTarget) close();
	}

	function setVerticalAlign() {
		if (verticalAlign === 'top') {
			return targetBoundingRect?.top
				? pxToRem(targetBoundingRect.top - contextMenuHeight - menuMargin)
				: undefined;
		}

		return targetBoundingRect?.top
			? pxToRem(targetBoundingRect.top + targetBoundingRect.height + menuMargin)
			: undefined;
	}

	function setHorizontalAlign() {
		if (horizontalAlign === 'left') {
			return targetBoundingRect?.left ? pxToRem(targetBoundingRect.left) : undefined;
		}

		return targetBoundingRect?.left
			? pxToRem(targetBoundingRect.left + targetBoundingRect.width - contextMenuWidth)
			: undefined;
	}

	function setTransformOrigin() {
		if (verticalAlign === 'top' && horizontalAlign === 'left') {
			return 'bottom left';
		}

		if (verticalAlign === 'top' && horizontalAlign === 'right') {
			return 'bottom right';
		}

		if (verticalAlign === 'bottom' && horizontalAlign === 'left') {
			return 'top left';
		}

		if (verticalAlign === 'bottom' && horizontalAlign === 'right') {
			return 'top right';
		}
	}

	onMount(() => {
		if (openByTarget) {
			// click listener for the target
			openByTarget.addEventListener('click', handleTargetClick);
		}

		return () => {
			if (openByTarget) {
				openByTarget.removeEventListener('click', handleTargetClick);
			}
		};
	});
</script>

{#if isVisibile}
	<div
		role="presentation"
		class="overlay-wrapper"
		use:portal={'body'}
		use:resizeObserver={getTargetBoundingRect}
		onclick={clickOutside}
	>
		<div
			bind:offsetHeight={contextMenuHeight}
			bind:offsetWidth={contextMenuWidth}
			class="context-menu"
			style:top={setVerticalAlign()}
			style:left={setHorizontalAlign()}
			style:transform-origin={setTransformOrigin()}
			style:--animation-transform-shift={verticalAlign === 'top' ? '6px' : '-6px'}
		>
			{@render children()}
		</div>
	</div>
{/if}

<style lang="postcss">
	.overlay-wrapper {
		z-index: var(--z-blocker);
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		/* background-color: rgba(0, 0, 0, 0.1); */
	}

	.context-menu {
		position: absolute;
		display: flex;
		flex-direction: column;
		background: var(--clr-bg-2);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		box-shadow: var(--fx-shadow-s);

		animation: fadeIn 0.12s ease-out forwards;
	}

	@keyframes fadeIn {
		0% {
			opacity: 0;
			transform: translateY(var(--animation-transform-shift)) scale(0.9);
		}
		50% {
			opacity: 1;
		}
		100% {
			opacity: 1;
			transform: translateY(0) scale(1);
		}
	}
</style>

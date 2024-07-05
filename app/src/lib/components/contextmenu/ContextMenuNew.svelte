<script lang="ts" context="module">
	export type ContextMenuActions = {
		open: (e?: MouseEvent, item?: any) => void;
		close: () => void;
	};
</script>

<script lang="ts">
	import { portal } from '$lib/utils/portal';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { resizeObserver } from '$lib/utils/resizeObserver';
	import { type Snippet } from 'svelte';

	// TYPES AND INTERFACES
	interface Props {
		trigger?: HTMLElement;
		rightClick?: boolean;
		verticalAlign?: 'top' | 'bottom';
		horizontalAlign?: 'left' | 'right';
		children: Snippet<[item: any]>;
	}

	const {
		trigger,
		rightClick,
		verticalAlign = 'bottom',
		horizontalAlign = 'right',
		children
	}: Props = $props();

	// LOCAL VARS
	let menuMargin = 4;

	// STATES
	let item = $state<any>();
	let contextMenuHeight = $state(0);
	let contextMenuWidth = $state(0);
	let isVisibile = $state(false);
	let menuPosition = $state({ x: 0, y: 0 });

	// METHODS
	export function close() {
		isVisibile = false;
	}

	export function open(e?: MouseEvent, newItem?: any) {
		if (!trigger) return;

		if (newItem) item = newItem;
		isVisibile = true;

		if (!rightClick) {
			setAlignByTarget();
		}

		if (rightClick && e) {
			menuPosition = {
				x: e.clientX,
				y: e.clientY
			};
		}
	}

	function setVerticalAlign(targetBoundingRect: DOMRect) {
		if (verticalAlign === 'top') {
			return targetBoundingRect?.top ? targetBoundingRect.top - contextMenuHeight - menuMargin : 0;
		}

		return targetBoundingRect?.top
			? targetBoundingRect.top + targetBoundingRect.height + menuMargin
			: 0;
	}

	function setHorizontalAlign(targetBoundingRect: DOMRect) {
		if (horizontalAlign === 'left') {
			return targetBoundingRect?.left ? targetBoundingRect.left : 0;
		}

		return targetBoundingRect?.left
			? targetBoundingRect.left + targetBoundingRect.width - contextMenuWidth
			: 0;
	}

	function setAlignByTarget() {
		if (trigger) {
			const targetBoundingRect = trigger.getBoundingClientRect();
			menuPosition = {
				x: setHorizontalAlign(targetBoundingRect),
				y: setVerticalAlign(targetBoundingRect)
			};
		}
	}

	function clickOutside(e: MouseEvent) {
		if (e.target === e.currentTarget) close();
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
</script>

{#if isVisibile}
	<div
		role="presentation"
		class="overlay-wrapper"
		use:portal={'body'}
		oncontextmenu={(e) => {
			e.preventDefault();
			close();
		}}
		use:resizeObserver={() => {
			if (!rightClick) setAlignByTarget();
		}}
		onclick={clickOutside}
	>
		<div
			bind:offsetHeight={contextMenuHeight}
			bind:offsetWidth={contextMenuWidth}
			class="context-menu"
			style:top={pxToRem(menuPosition.y)}
			style:left={pxToRem(menuPosition.x)}
			style:transform-origin={setTransformOrigin()}
			style:--animation-transform-shift={verticalAlign === 'top' ? '6px' : '-6px'}
		>
			{@render children(item)}
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

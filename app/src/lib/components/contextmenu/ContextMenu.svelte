<script lang="ts">
	import { clickOutside } from '$lib/clickOutside';
	import { portal } from '$lib/utils/portal';
	import { resizeObserver } from '$lib/utils/resizeObserver';
	import { type Snippet } from 'svelte';

	// TYPES AND INTERFACES
	interface Props {
		target?: HTMLElement;
		openByMouse?: boolean;
		verticalAlign?: 'top' | 'bottom';
		horizontalAlign?: 'left' | 'right';
		children: Snippet<[item: any]>;
		onclose?: () => void;
		onopen?: () => void;
	}

	const {
		target,
		openByMouse,
		verticalAlign = 'bottom',
		horizontalAlign = 'right',
		children,
		onclose,
		onopen
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
		onclose && onclose();
	}

	export function open(e?: MouseEvent, newItem?: any) {
		if (!target) return;

		if (newItem) item = newItem;
		isVisibile = true;
		onopen && onopen();

		if (!openByMouse) {
			setAlignByTarget();
		}

		if (openByMouse && e) {
			menuPosition = {
				x: e.clientX,
				y: e.clientY
			};
		}
	}

	export function toggle(e?: MouseEvent, newItem?: any) {
		if (!isVisibile) {
			open(e, newItem);
		} else {
			close();
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
		if (target) {
			const targetBoundingRect = target.getBoundingClientRect();
			menuPosition = {
				x: setHorizontalAlign(targetBoundingRect),
				y: setVerticalAlign(targetBoundingRect)
			};
		}
	}

	function setTransformOrigin() {
		if (!openByMouse) {
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
		} else {
			return 'top left';
		}
	}
</script>

{#snippet contextMenu()}
	<div
		use:clickOutside={{
			excludeElement: target,
			handler: () => close()
		}}
		use:resizeObserver={() => {
			if (!openByMouse) setAlignByTarget();
		}}
		bind:offsetHeight={contextMenuHeight}
		bind:offsetWidth={contextMenuWidth}
		class="context-menu"
		style:top="{menuPosition.y}px"
		style:left="{menuPosition.x}px"
		style:transform-origin={setTransformOrigin()}
		style:--animation-transform-shift={verticalAlign === 'top' ? '6px' : '-6px'}
	>
		{@render children(item)}
	</div>
{/snippet}

{#if isVisibile}
	<div class="portal-wrap" use:portal={'body'}>
		{#if openByMouse}
			{@render contextMenu()}
		{:else}
			<div class="overlay-wrapper">
				{@render contextMenu()}
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.portal-wrap {
		display: contents;
	}

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
		z-index: var(--z-blocker);
		position: fixed;
		display: flex;
		flex-direction: column;
		background: var(--clr-bg-2);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		box-shadow: var(--fx-shadow-s);

		animation: fadeIn 0.1s ease-out forwards;
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

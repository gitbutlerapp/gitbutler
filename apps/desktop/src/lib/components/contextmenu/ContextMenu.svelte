<script lang="ts">
	import { clickOutside } from '$lib/clickOutside';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { focusTrap } from '@gitbutler/ui/utils/focusTrap';
	import { portal } from '@gitbutler/ui/utils/portal';
	import { type Snippet } from 'svelte';

	interface Props {
		target?: HTMLElement;
		openByMouse?: boolean;
		verticalAlign?: 'top' | 'bottom';
		horizontalAlign?: 'left' | 'right';
		children: Snippet<[item: any]>;
		onclose?: () => void;
		onopen?: () => void;
	}

	let {
		target,
		openByMouse,
		verticalAlign = 'bottom',
		horizontalAlign = 'right',
		children,
		onclose,
		onopen
	}: Props = $props();

	let el: HTMLElement | undefined = $state();
	let item = $state<any>();
	let contextMenuHeight = $state(0);
	let contextMenuWidth = $state(0);
	let isVisible = $state(false);
	let menuPosition = $state({ x: 0, y: 0 });
	let savedMouseEvent: MouseEvent | undefined;

	function setVerticalAlign(targetBoundingRect: DOMRect) {
		if (verticalAlign === 'top') {
			return targetBoundingRect?.top ? targetBoundingRect.top - contextMenuHeight : 0;
		}

		return targetBoundingRect?.top ? targetBoundingRect.top + targetBoundingRect.height : 0;
	}

	function setHorizontalAlign(targetBoundingRect: DOMRect) {
		if (horizontalAlign === 'left') {
			return targetBoundingRect?.left ? targetBoundingRect.left : 0;
		}

		console.log('left', targetBoundingRect.left, targetBoundingRect.width, contextMenuWidth);
		return targetBoundingRect?.left
			? targetBoundingRect.left + targetBoundingRect.width - contextMenuWidth
			: 0;
	}

	function setAlignByMouse(e?: MouseEvent) {
		if (!e) return;

		let newMenuPosition = { x: e.clientX, y: e.clientY };

		const menuOffset = 20;

		// Check if the menu exceeds the window's right edge
		const exceedsRight = newMenuPosition.x + contextMenuWidth > window.innerWidth;
		if (exceedsRight) {
			newMenuPosition.x = window.innerWidth - contextMenuWidth - menuOffset;
		}

		// Check if the menu exceeds the window's left edge
		const exceedsLeft = newMenuPosition.x < 0;
		if (exceedsLeft) {
			newMenuPosition.x = 0;
		}

		// Check if the menu exceeds the window's bottom edge
		const exceedsBottom = newMenuPosition.y + contextMenuHeight > window.innerHeight;
		console.log('e', newMenuPosition.y, window.innerHeight, contextMenuHeight);
		if (exceedsBottom) {
			newMenuPosition.y = window.innerHeight - contextMenuHeight - menuOffset;
		}

		// Check if the menu exceeds the window's top edge
		const exceedsTop = newMenuPosition.y < 0;
		if (exceedsTop) {
			newMenuPosition.y = 0;
		}

		// Apply the new position
		menuPosition = newMenuPosition;
	}

	function setAlignByTarget() {
		if (!target) return;

		const targetBoundingRect = target.getBoundingClientRect();
		let newMenuPosition = {
			x: setHorizontalAlign(targetBoundingRect),
			y: setVerticalAlign(targetBoundingRect)
		};

		// Check if the menu goes beyond the window's right edge
		const exceedsRight = newMenuPosition.x + contextMenuWidth > window.innerWidth;
		if (exceedsRight) {
			horizontalAlign = horizontalAlign === 'right' ? 'left' : 'right';
			newMenuPosition.x = setHorizontalAlign(targetBoundingRect);
		}

		// Check if the menu goes beyond the window's left edge
		const exceedsLeft = newMenuPosition.x < 0;
		if (exceedsLeft) {
			horizontalAlign = 'right';
			newMenuPosition.x = setHorizontalAlign(targetBoundingRect);
		}

		// Check if the menu goes beyond the window's bottom edge
		const exceedsBottom = newMenuPosition.y + contextMenuHeight > window.innerHeight;
		if (exceedsBottom) {
			verticalAlign = verticalAlign === 'bottom' ? 'top' : 'bottom';
			newMenuPosition.y = setVerticalAlign(targetBoundingRect);
		}

		// Check if the menu goes beyond the window's top edge
		const exceedsTop = newMenuPosition.y < 0;
		if (exceedsTop) {
			verticalAlign = 'bottom';
			newMenuPosition.y = setVerticalAlign(targetBoundingRect);
		}

		// Apply the new position
		menuPosition = newMenuPosition;
	}

	export function open(e?: MouseEvent, newItem?: any) {
		if (!target) return;

		if (newItem) {
			item = newItem;
		}

		isVisible = true;
		onopen?.();

		if (!openByMouse) {
			setAlignByTarget();
		}

		if (openByMouse && e) {
			savedMouseEvent = e;
		}
	}

	export function close() {
		if (!isVisible) return;

		isVisible = false;
		onclose?.();
	}

	export function toggle(e?: MouseEvent, newItem?: any) {
		if (!isVisible) {
			open(e, newItem);
		} else {
			close();
		}
	}

	$effect(() => {
		if (isVisible) {
			if (contextMenuHeight > 0 && contextMenuWidth > 0) {
				el?.focus();

				if (openByMouse) {
					setAlignByMouse(savedMouseEvent);
				} else {
					setAlignByTarget();
				}
			}
		} else {
			savedMouseEvent = undefined;
		}
	});

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

	const handleKeyDown = createKeybind({
		Escape: () => {
			if (isVisible) {
				close();
			}
		}
	});
</script>

<svelte:window on:keydown={handleKeyDown} />

{#snippet contextMenu()}
	<div
		bind:this={el}
		tabindex="-1"
		use:focusTrap
		use:clickOutside={{
			handler: () => close()
		}}
		bind:clientHeight={contextMenuHeight}
		bind:clientWidth={contextMenuWidth}
		class="context-menu"
		class:top-oriented={verticalAlign === 'top'}
		class:bottom-oriented={verticalAlign === 'bottom'}
		style:top="{menuPosition.y}px"
		style:left="{menuPosition.x}px"
		style:transform-origin={setTransformOrigin()}
		style:--animation-transform-shift={verticalAlign === 'top' ? '6px' : '-6px'}
	>
		{@render children(item)}
	</div>
{/snippet}

{#if isVisible}
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

	.top-oriented {
		margin-top: -6px;
	}

	.bottom-oriented {
		margin-top: 4px;
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
		outline: none;

		animation: fadeIn 0.08s ease-out forwards;
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

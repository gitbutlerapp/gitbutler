<script lang="ts">
	import { createKeybind } from '$lib/utils/hotkeys';
	import { clickOutside } from '@gitbutler/ui/utils/clickOutside';
	import { focusTrap } from '@gitbutler/ui/utils/focusTrap';
	import { portal } from '@gitbutler/ui/utils/portal';
	import { type Snippet } from 'svelte';

	interface Props {
		leftClickTrigger?: HTMLElement;
		rightClickTrigger?: HTMLElement;
		verticalAlign?: 'top' | 'bottom';
		horizontalAlign?: 'left' | 'right';
		children: Snippet<[item: any]>;
		onclose?: () => void;
		onopen?: () => void;
		ontoggle?: (isOpen: boolean, isLeftClick: boolean) => void;
	}

	let {
		leftClickTrigger,
		rightClickTrigger,
		verticalAlign = 'bottom',
		horizontalAlign = 'right',
		children,
		onclose,
		onopen,
		ontoggle
	}: Props = $props();

	let menuContainer: HTMLElement | undefined = $state();
	let item = $state<any>();
	let contextMenuHeight = $state(0);
	let contextMenuWidth = $state(0);
	let isVisible = $state(false);
	let menuPosition = $state({ x: 0, y: 0 });
	let savedMouseEvent: MouseEvent | undefined = $state();

	function setVerticalAlign(targetBoundingRect: DOMRect) {
		return verticalAlign === 'top'
			? targetBoundingRect?.top
				? targetBoundingRect.top - contextMenuHeight
				: 0
			: targetBoundingRect?.top
				? targetBoundingRect.top + targetBoundingRect.height
				: 0;
	}

	function setHorizontalAlign(targetBoundingRect: DOMRect) {
		const correction = 2;
		return horizontalAlign === 'left'
			? targetBoundingRect?.left ?? 0
			: (targetBoundingRect?.left ?? 0) + targetBoundingRect.width - contextMenuWidth - correction;
	}

	function executeByTrigger(callback: (isOpened: boolean, isLeftClick: boolean) => void) {
		if (leftClickTrigger && !savedMouseEvent) {
			callback(isVisible, true);
		} else if (rightClickTrigger && savedMouseEvent) {
			callback(isVisible, false);
		}
	}

	function setAlignByMouse(
		e?: MouseEvent,
		contextMenuWidth: number = 0,
		contextMenuHeight: number = 0
	) {
		if (!e) return;
		let newMenuPosition = { x: e.clientX, y: e.clientY };
		const menuWindowEdgesOffset = 20;

		// Adjust menu position to stay within the window
		if (newMenuPosition.x + contextMenuWidth > window.innerWidth) {
			newMenuPosition.x = window.innerWidth - contextMenuWidth - menuWindowEdgesOffset;
		}
		if (newMenuPosition.x < 0) newMenuPosition.x = 0;
		if (newMenuPosition.y + contextMenuHeight > window.innerHeight) {
			newMenuPosition.y = window.innerHeight - contextMenuHeight - menuWindowEdgesOffset;
		}
		if (newMenuPosition.y < 0) newMenuPosition.y = 0;

		menuPosition = newMenuPosition;
	}

	function setAlignByTarget(target: HTMLElement) {
		const targetBoundingRect = target.getBoundingClientRect();
		let newMenuPosition = {
			x: setHorizontalAlign(targetBoundingRect),
			y: setVerticalAlign(targetBoundingRect)
		};

		// Adjust alignment to stay within the window
		if (newMenuPosition.x + contextMenuWidth > window.innerWidth) {
			horizontalAlign = horizontalAlign === 'right' ? 'left' : 'right';
			newMenuPosition.x = setHorizontalAlign(targetBoundingRect);
		}
		if (newMenuPosition.x < 0) {
			horizontalAlign = 'right';
			newMenuPosition.x = setHorizontalAlign(targetBoundingRect);
		}
		if (newMenuPosition.y + contextMenuHeight > window.innerHeight) {
			verticalAlign = verticalAlign === 'bottom' ? 'top' : 'bottom';
			newMenuPosition.y = setVerticalAlign(targetBoundingRect);
		}
		if (newMenuPosition.y < 0) {
			verticalAlign = 'bottom';
			newMenuPosition.y = setVerticalAlign(targetBoundingRect);
		}

		menuPosition = newMenuPosition;
	}

	export function open(e?: MouseEvent, newItem?: any) {
		if (!(leftClickTrigger || rightClickTrigger)) return;

		item = newItem ?? item;
		isVisible = true;
		savedMouseEvent = e;

		onopen?.();
		if (ontoggle) executeByTrigger(ontoggle);
	}

	export function close() {
		if (!isVisible) return;
		isVisible = false;
		// leftClickTrigger?.style.removeProperty('background');
		onclose?.();
		if (ontoggle) executeByTrigger(ontoggle);
	}

	export function toggle(e?: MouseEvent, newItem?: any) {
		if (isVisible) {
			close();
		} else {
			open(e, newItem);
		}
	}

	$effect(() => {
		if (isVisible && contextMenuHeight > 0 && contextMenuWidth > 0) {
			menuContainer?.focus();

			if (savedMouseEvent && rightClickTrigger) {
				setAlignByMouse(savedMouseEvent, contextMenuWidth, contextMenuHeight);
			} else if (leftClickTrigger) {
				// leftClickTrigger.style.background = 'red';
				setAlignByTarget(leftClickTrigger);
			}
		}

		if (!isVisible) {
			savedMouseEvent = undefined;
		}
	});

	function setTransformOrigin() {
		if (savedMouseEvent) return 'top left';
		if (verticalAlign === 'top') return horizontalAlign === 'left' ? 'bottom left' : 'bottom right';
		return horizontalAlign === 'left' ? 'top left' : 'top right';
	}

	const handleKeyDown = createKeybind({
		Escape: () => {
			if (isVisible) close();
		}
	});
</script>

<svelte:window on:keydown={handleKeyDown} />

{#snippet contextMenu()}
	<div
		bind:this={menuContainer}
		tabindex="-1"
		use:focusTrap
		use:clickOutside={{
			excludeElement: !savedMouseEvent ? leftClickTrigger ?? rightClickTrigger : undefined,
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
		{@render contextMenu()}
	</div>
{/if}

<style lang="postcss">
	.portal-wrap {
		display: contents;
	}
	.top-oriented {
		margin-top: -6px;
	}
	.bottom-oriented {
		margin-top: 4px;
	}
	.context-menu {
		pointer-events: none;
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
			pointer-events: all;
		}
	}
</style>

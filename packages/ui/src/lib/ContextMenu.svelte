<script lang="ts">
	import { clickOutside } from '@gitbutler/ui/utils/clickOutside';
	import { focusTrap } from '@gitbutler/ui/utils/focusTrap';
	import { portal } from '@gitbutler/ui/utils/portal';
	import { type Snippet } from 'svelte';

	interface BaseProps {
		children: Snippet<[item: any]>;
		leftClickTrigger?: HTMLElement;
		rightClickTrigger?: HTMLElement;
		onclose?: () => void;
		onopen?: () => void;
		ontoggle?: (isOpen: boolean, isLeftClick: boolean) => void;
	}

	type HorizontalProps = BaseProps & {
		side?: 'top' | 'bottom';
		horizontalAlign?: 'left' | 'right';
		verticalAlign?: never;
	};

	type VerticalProps = BaseProps & {
		side?: 'left' | 'right';
		verticalAlign?: 'top' | 'bottom';
		horizontalAlign?: never;
	};

	type Props = HorizontalProps | VerticalProps;

	let {
		leftClickTrigger,
		rightClickTrigger,
		side = 'bottom',
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
		if (['top', 'bottom'].includes(side)) {
			return side === 'top'
				? targetBoundingRect.top
					? targetBoundingRect.top - contextMenuHeight
					: 0
				: targetBoundingRect.top
					? targetBoundingRect.top + targetBoundingRect.height
					: 0;
		} else if (['left', 'right'].includes(side)) {
			if (verticalAlign === 'top') {
				return targetBoundingRect.bottom - targetBoundingRect.height;
			} else if (verticalAlign === 'bottom') {
				return targetBoundingRect.bottom - contextMenuHeight;
			}
		}
		return 0;
	}

	function setHorizontalAlign(targetBoundingRect: DOMRect) {
		const padding = 2;

		if (['top', 'bottom'].includes(side)) {
			return horizontalAlign === 'left'
				? targetBoundingRect.left
				: targetBoundingRect.left + targetBoundingRect.width - contextMenuWidth - padding;
		} else if (['left', 'right'].includes(side)) {
			if (side === 'left') {
				return targetBoundingRect.x - contextMenuWidth - padding * 2;
			} else {
				return targetBoundingRect.right + padding;
			}
		}
		return padding;
	}

	function executeByTrigger(callback: (isOpened: boolean, isLeftClick: boolean) => void) {
		if (leftClickTrigger && !savedMouseEvent) {
			callback(isVisible, true);
		} else if (rightClickTrigger && savedMouseEvent) {
			callback(isVisible, false);
		}
	}

	function setAlignByMouse(e?: MouseEvent) {
		if (!e) return;
		menuPosition = { x: e.clientX, y: e.clientY };
	}

	function setAlignByTarget(target: HTMLElement) {
		const targetBoundingRect = target.getBoundingClientRect();
		let newMenuPosition = {
			x: setHorizontalAlign(targetBoundingRect),
			y: setVerticalAlign(targetBoundingRect)
		};

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

	function setAlignment() {
		if (savedMouseEvent && rightClickTrigger) {
			setAlignByMouse(savedMouseEvent);
		} else if (leftClickTrigger) {
			setAlignByTarget(leftClickTrigger);
		}
	}

	$effect(() => {
		if (!isVisible || !menuContainer) return;

		setAlignment();

		// Keep contextMenu in viewport
		const observer = new IntersectionObserver(
			(entries) => {
				const entry = entries[0];
				if (!entry.isIntersecting) {
					const rect = entry.boundingClientRect;
					const viewport = entry.rootBounds;
					if (!viewport) return;

					if (rect.right > viewport.right) {
						horizontalAlign = 'right';
						setAlignment();
					}
					if (rect.left < viewport.left) {
						horizontalAlign = 'left';
						setAlignment();
					}
					if (rect.bottom > viewport.bottom) {
						side = 'top';
						setAlignment();
					}
					if (rect.top < viewport.top) {
						side = 'bottom';
						setAlignment();
					}
				}
			},
			{
				root: null,
				rootMargin: '0px',
				threshold: 1.0
			}
		);

		observer.observe(menuContainer);
		return () => observer.disconnect();
	});

	function setTransformOrigin() {
		if (savedMouseEvent) return 'top left';
		if (['top', 'bottom'].includes(side))
			return horizontalAlign === 'left' ? `${side} left` : `${side} right`;
		if (['left', 'right'].includes(side))
			return verticalAlign === 'top' ? `top ${side}` : `bottom ${side}`;
		return horizontalAlign === 'left' ? 'top left' : 'top right';
	}

	export function isOpen() {
		return isVisible;
	}
</script>

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
		class:top-oriented={side === 'top'}
		class:bottom-oriented={side === 'bottom'}
		style:top="{menuPosition.y}px"
		style:left="{menuPosition.x}px"
		style:transform-origin={setTransformOrigin()}
		style:--animation-transform-shift={side === 'top' ? '6px' : '-6px'}
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
		min-width: 128px;
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

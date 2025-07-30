<script lang="ts" generics="T = any">
	import { clickOutside } from '$lib/utils/clickOutside';
	import { focusTrap } from '$lib/utils/focusTrap';
	import { portal } from '$lib/utils/portal';
	import { type Snippet } from 'svelte';

	// Constants
	const POSITIONING = {
		PADDING: 2,
		MARGIN_TOP: -6,
		MARGIN_BOTTOM: 4,
		MARGIN_LEFT: -2,
		VIEWPORT_ADJUSTMENT_DELAY: 0,
		ANIMATION_SHIFT: '6px'
	} as const;

	interface BaseProps<T = any> {
		testId?: string;
		children?: Snippet<[item: T]>;
		leftClickTrigger?: HTMLElement;
		rightClickTrigger?: HTMLElement;
		onclose?: () => void;
		onopen?: () => void;
		ontoggle?: (isOpen: boolean, isLeftClick: boolean) => void;
		onclick?: () => void;
		onkeypress?: () => void;
		menu?: Snippet<[{ close: () => void }]>;
	}

	type HorizontalProps<T = any> = BaseProps<T> & {
		side?: 'top' | 'bottom';
		horizontalAlign?: 'left' | 'right';
		verticalAlign?: never;
	};

	type VerticalProps<T = any> = BaseProps<T> & {
		side?: 'left' | 'right';
		verticalAlign?: 'top' | 'bottom';
		horizontalAlign?: never;
	};

	type Props<T = any> = HorizontalProps<T> | VerticalProps<T>;

	let {
		testId,
		leftClickTrigger,
		rightClickTrigger,
		side = 'bottom',
		verticalAlign = 'bottom',
		horizontalAlign = 'right',
		children,
		onclose,
		onopen,
		ontoggle,
		onclick,
		onkeypress,
		menu
	}: Props<T> = $props();

	let menuContainer: HTMLElement | undefined = $state();
	let item = $state<T>();
	let contextMenuHeight = $state(0);
	let contextMenuWidth = $state(0);
	let isVisible = $state(false);
	let menuPosition = $state({ x: 0, y: 0 });
	let savedMouseEvent: MouseEvent | undefined = $state();

	// Store the original/default side value to fall back to when there's no space in either direction
	let originalSide = side;

	function calculateVerticalPosition(targetBoundingRect: DOMRect): number {
		// For horizontal sides (top/bottom)
		if (side === 'top' || side === 'bottom') {
			if (side === 'top') {
				return targetBoundingRect.top > 0 ? targetBoundingRect.top - contextMenuHeight : 0;
			}
			return targetBoundingRect.top > 0 ? targetBoundingRect.top + targetBoundingRect.height : 0;
		}

		// For vertical sides (left/right)
		if (verticalAlign === 'top') {
			return targetBoundingRect.bottom - targetBoundingRect.height;
		}
		if (verticalAlign === 'bottom') {
			return targetBoundingRect.bottom - contextMenuHeight;
		}

		return 0;
	}

	function calculateHorizontalPosition(targetBoundingRect: DOMRect): number {
		// For horizontal sides (top/bottom)
		if (side === 'top' || side === 'bottom') {
			return horizontalAlign === 'left'
				? targetBoundingRect.left
				: targetBoundingRect.left +
						targetBoundingRect.width -
						contextMenuWidth -
						POSITIONING.PADDING;
		}

		// For vertical sides (left/right)
		if (side === 'left') {
			return targetBoundingRect.x - contextMenuWidth - POSITIONING.PADDING * 2;
		}
		if (side === 'right') {
			return targetBoundingRect.right + POSITIONING.PADDING;
		}

		return POSITIONING.PADDING;
	}

	function executeByTrigger(callback: (isOpened: boolean, isLeftClick: boolean) => void) {
		const isLeftClick = Boolean(leftClickTrigger && !savedMouseEvent);
		const isRightClick = Boolean(rightClickTrigger && savedMouseEvent);

		if (isLeftClick || isRightClick) {
			callback(isVisible, isLeftClick);
		}
	}

	function setAlignByMouse(e?: MouseEvent) {
		if (!e) return;

		const clientX = horizontalAlign === 'left' ? e.clientX - contextMenuWidth : e.clientX;
		const clientY = side === 'top' ? e.clientY - contextMenuHeight : e.clientY;
		menuPosition = { x: clientX, y: clientY };
	}

	function setAlignByTarget(target: HTMLElement) {
		const targetBoundingRect = target.getBoundingClientRect();
		menuPosition = {
			x: calculateHorizontalPosition(targetBoundingRect),
			y: calculateVerticalPosition(targetBoundingRect)
		};
	}

	export function open(e?: MouseEvent, newItem?: T) {
		if (!(leftClickTrigger || rightClickTrigger)) return;

		// Reset to original values when opening
		originalSide = side;

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

	export function toggle(e?: MouseEvent, newItem?: T) {
		if (isVisible) {
			close();
		} else {
			open(e, newItem);
		}
	}

	function calculateBestSide(): 'top' | 'bottom' | 'left' | 'right' {
		if (!leftClickTrigger || !contextMenuHeight) return originalSide;

		const targetRect = leftClickTrigger.getBoundingClientRect();
		const viewport = {
			width: window.innerWidth,
			height: window.innerHeight
		};

		// For horizontal sides (top/bottom)
		if (originalSide === 'top' || originalSide === 'bottom') {
			const spaceBelow = viewport.height - targetRect.bottom;
			const spaceAbove = targetRect.top;

			// Check if menu fits in preferred position
			if (originalSide === 'bottom' && spaceBelow >= contextMenuHeight) {
				return 'bottom';
			}
			if (originalSide === 'top' && spaceAbove >= contextMenuHeight) {
				return 'top';
			}

			// Try alternative position
			if (originalSide === 'bottom' && spaceAbove >= contextMenuHeight) {
				return 'top';
			}
			if (originalSide === 'top' && spaceBelow >= contextMenuHeight) {
				return 'bottom';
			}

			// No space in either direction, use original
			return originalSide;
		}

		return originalSide;
	}

	function setAlignment() {
		if (savedMouseEvent && rightClickTrigger) {
			setAlignByMouse(savedMouseEvent);
		} else if (leftClickTrigger) {
			// Calculate the best side before positioning
			side = calculateBestSide();
			setAlignByTarget(leftClickTrigger);
		}
	}

	$effect(() => {
		if (!isVisible || !menuContainer) return;

		setAlignment();

		// Simple horizontal viewport adjustment
		const observer = new IntersectionObserver(
			(entries) => {
				const entry = entries[0];
				if (!entry.isIntersecting) {
					const rect = entry.boundingClientRect;
					const viewport = entry.rootBounds;
					if (!viewport) return;

					let needsRealignment = false;

					// Only horizontal adjustments to prevent flickering
					if (rect.right > viewport.right && horizontalAlign !== 'right') {
						horizontalAlign = 'right';
						needsRealignment = true;
					}
					if (rect.left < viewport.left && horizontalAlign !== 'left') {
						horizontalAlign = 'left';
						needsRealignment = true;
					}

					if (needsRealignment) {
						setAlignment(); // Skip side calculation during horizontal adjustments
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

	function getTransformOrigin(): string {
		// Right-click context menus grow from cursor position
		if (savedMouseEvent) {
			return 'top left';
		}

		// Calculate origin based on side and alignment
		const verticalOrigin =
			side === 'top'
				? 'bottom'
				: side === 'bottom'
					? 'top'
					: verticalAlign === 'top'
						? 'top'
						: 'bottom';

		const horizontalOrigin =
			side === 'left'
				? 'right'
				: side === 'right'
					? 'left'
					: horizontalAlign === 'left'
						? 'left'
						: 'right';

		return `${verticalOrigin} ${horizontalOrigin}`;
	}

	export function isOpen() {
		return isVisible;
	}

	function handleKeyNavigation(e: KeyboardEvent) {
		switch (e.key) {
			case 'Escape':
				e.preventDefault();
				close();
				break;
			case 'ArrowDown':
			case 'ArrowUp':
				e.preventDefault();
				// Focus management is handled by focusTrap utility
				// This prevents default browser behavior
				break;
			case 'Enter':
			case ' ':
				// Allow default behavior for menu item activation
				break;
		}
	}
</script>

{#if isVisible}
	<div class="portal-wrap" use:portal={'body'}>
		<!-- svelte-ignore a11y_autofocus -->
		<div
			data-testid={testId}
			bind:this={menuContainer}
			tabindex="-1"
			use:focusTrap
			autofocus
			use:clickOutside={{
				excludeElement: !savedMouseEvent ? (leftClickTrigger ?? rightClickTrigger) : undefined,
				handler: () => close()
			}}
			bind:clientHeight={contextMenuHeight}
			bind:clientWidth={contextMenuWidth}
			{onclick}
			{onkeypress}
			onkeydown={handleKeyNavigation}
			class="context-menu hide-native-scrollbar"
			class:top-oriented={side === 'top'}
			class:bottom-oriented={side === 'bottom'}
			class:left-oriented={side === 'left'}
			class:right-oriented={side === 'right'}
			style:top="{menuPosition.y}px"
			style:left="{menuPosition.x}px"
			style:transform-origin={getTransformOrigin()}
			style:--animation-transform-y-shift={side === 'top'
				? POSITIONING.ANIMATION_SHIFT
				: side === 'bottom'
					? `-${POSITIONING.ANIMATION_SHIFT}`
					: '0'}
			role="menu"
		>
			{@render children?.(item as T)}
			<!-- TODO: refactor `children` and combine with this snippet. -->
			{@render menu?.({ close })}
		</div>
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
	.left-oriented {
		margin-left: -2px;
	}
	.right-oriented {
		margin-left: 2px;
	}
	.context-menu {
		display: flex;
		z-index: var(--z-blocker);
		position: fixed;
		flex-direction: column;
		min-width: 128px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		outline: none;
		background: var(--clr-bg-2);
		background-color: red;
		box-shadow: var(--fx-shadow-s);
		animation: fadeIn 0.08s ease-out forwards;
		pointer-events: none;
	}
	@keyframes fadeIn {
		0% {
			transform: translateY(var(--animation-transform-y-shift)) scale(0.9);
			opacity: 0;
		}
		50% {
			opacity: 1;
		}
		100% {
			transform: scale(1);
			opacity: 1;
			pointer-events: all;
		}
	}
</style>

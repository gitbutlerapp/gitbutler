<script lang="ts">
	import { clickOutside } from '$lib/utils/clickOutside';
	import { focusTrap } from '$lib/utils/focusTrap';
	import { portal } from '$lib/utils/portal';
	import { type Snippet } from 'svelte';

	interface BaseProps {
		testId?: string;
		children?: Snippet<[item: any]>;
		leftClickTrigger?: HTMLElement;
		rightClickTrigger?: HTMLElement;
		onclose?: () => void;
		onopen?: () => void;
		ontoggle?: (isOpen: boolean, isLeftClick: boolean) => void;
		onclick?: () => void;
		onkeypress?: () => void;
		menu?: Snippet<[{ close: () => void }]>;
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
		const clientX = horizontalAlign === 'left' ? e.clientX - contextMenuWidth : e.clientX;
		const clientY = side === 'top' ? e.clientY - contextMenuHeight : e.clientY;

		menuPosition = { x: clientX, y: clientY };
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
		let repositionTimeout: number | undefined;
		let hasRepositioned = false;

		const observer = new IntersectionObserver(
			(entries) => {
				const entry = entries[0];
				if (!entry.isIntersecting && !hasRepositioned) {
					const rect = entry.boundingClientRect;
					const viewport = entry.rootBounds;
					if (!viewport) return;

					// Clear any pending repositioning
					if (repositionTimeout) {
						clearTimeout(repositionTimeout);
					}

					// Debounce repositioning to prevent flickering
					repositionTimeout = setTimeout(() => {
						hasRepositioned = true;
						
						if (rect.right > viewport.right) {
							horizontalAlign = 'right';
							setAlignment();
						} else if (rect.left < viewport.left) {
							horizontalAlign = 'left';
							setAlignment();
						}
						
						if (rect.bottom > viewport.bottom && rect.top > viewport.top) {
							side = 'top';
							setAlignment();
						} else if (rect.top < viewport.top) {
							side = 'bottom';
							setAlignment();
						}
					}, 16); // Single frame delay to prevent rapid repositioning
				}
			},
			{
				root: null,
				rootMargin: '0px',
				threshold: 1.0
			}
		);

		observer.observe(menuContainer);
		return () => {
			observer.disconnect();
			if (repositionTimeout) {
				clearTimeout(repositionTimeout);
			}
		};
	});

	function setTransformOrigin() {
		// if trigger is right click, grow from cursor
		if (savedMouseEvent) return 'top left';

		// if attaching to a trigger element
		if (['top', 'bottom'].includes(side)) {
			return horizontalAlign === 'left'
				? `${side === 'top' ? 'bottom' : 'top'} left`
				: `${side === 'top' ? 'bottom' : 'top'} right`;
		}

		if (['left', 'right'].includes(side)) {
			return verticalAlign === 'top'
				? `top ${side === 'left' ? 'right' : 'left'}`
				: `bottom ${side === 'left' ? 'right' : 'left'}`;
		}

		return horizontalAlign === 'left' ? 'top left' : 'top right';
	}

	export function isOpen() {
		return isVisible;
	}

	function handleKeyNavigation(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			close();
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
			class="context-menu"
			class:top-oriented={side === 'top'}
			class:bottom-oriented={side === 'bottom'}
			class:left-oriented={side === 'left'}
			class:right-oriented={side === 'right'}
			style:top="{menuPosition.y}px"
			style:left="{menuPosition.x}px"
			style:transform-origin={setTransformOrigin()}
			style:--animation-transform-y-shift={side === 'top'
				? '6px'
				: side === 'bottom'
					? '-6px'
					: '0'}
			role="menu"
		>
			{@render children?.(item)}
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

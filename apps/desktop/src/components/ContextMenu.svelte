<script lang="ts">
	import { clickOutside } from '@gitbutler/ui/utils/clickOutside';
	import { focusTrap } from '@gitbutler/ui/utils/focusTrap';
	import { portal } from '@gitbutler/ui/utils/portal';
	import { type Snippet } from 'svelte';

	type BaseProps = {
		testId?: string;
		children?: Snippet<[item: any]>;
		onclose?: () => void;
		onopen?: () => void;
		onclick?: () => void;
		onkeypress?: () => void;
		menu?: Snippet<[{ close: () => void }]>;
		position: { coords?: { x: number; y: number }; element?: HTMLElement };
	};

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
		side = 'bottom',
		verticalAlign = 'bottom',
		horizontalAlign = 'right',
		children,
		onclose,
		onclick,
		onkeypress,
		menu,
		position
	}: Props = $props();

	let menuContainer = $state<HTMLElement>();
	let item = $state<any>();
	let contextMenuHeight = $state(0);
	let contextMenuWidth = $state(0);
	let menuPosition = $state<{ x: number; y: number }>();
	let savedMouseEvent: MouseEvent | undefined = $state();

	function getVerticalAlign(targetBoundingRect: DOMRect) {
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

	function getHorizontalAlign(targetBoundingRect: DOMRect) {
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

	function getPositionFromAnchor(element: HTMLElement) {
		const targetBoundingRect = element.getBoundingClientRect();
		return {
			x: getHorizontalAlign(targetBoundingRect),
			y: getVerticalAlign(targetBoundingRect)
		};
	}

	function getPositionFromCoords(position: { x: number; y: number }) {
		if (menuContainer) {
			let x =
				position.x + menuContainer?.offsetWidth > window.innerWidth
					? position.x - menuContainer.offsetWidth
					: position.x;
			let y =
				position.y + menuContainer?.offsetHeight > window.innerHeight
					? position.y - menuContainer.offsetHeight
					: position.y;
			return { x, y };
		}
		return position;
	}

	function close() {
		onclose?.();
	}

	function setAlignment() {
		if (position.element) {
			menuPosition = getPositionFromAnchor(position.element);
		} else if (position.coords) {
			menuPosition = getPositionFromCoords(position.coords);
		}
	}

	$effect(() => {
		if (!menuContainer) return;

		setAlignment();

		// Keep contextMenu in viewport
		let repositionTimeout: number | undefined;
		let hasRepositioned = false;

		const observer = new IntersectionObserver(
			(entries) => {
				const entry = entries[0]!;
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
						
						if (rect.bottom > viewport.bottom) {
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

	function handleKeyNavigation(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			close();
		}
	}
</script>

<div class="portal-wrap" use:portal={'body'}>
	<!-- svelte-ignore a11y_autofocus -->
	<div
		data-testid={testId}
		bind:this={menuContainer}
		tabindex="-1"
		use:focusTrap
		autofocus
		use:clickOutside={{
			excludeElement: position.element,
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
		style:top={menuPosition?.y + 'px'}
		style:left={menuPosition?.x + 'px'}
		style:transform-origin={setTransformOrigin()}
		style:--animation-transform-y-shift={side === 'top' ? '6px' : side === 'bottom' ? '-6px' : '0'}
		role="menu"
	>
		{@render children?.(item)}
		<!-- TODO: refactor `children` and combine with this snippet. -->
		{@render menu?.({ close })}
	</div>
</div>

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

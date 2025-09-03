<script lang="ts" generics="T = any">
	import { focusTrap } from '$lib/utils/focusTrap';
	import { menuManager } from '$lib/utils/menuManager';
	import { portal } from '$lib/utils/portal';
	import { setContext, onDestroy } from 'svelte';
	import { type Snippet } from 'svelte';

	// Context key for submenu coordination
	const SUBMENU_CONTEXT_KEY = 'contextmenu-submenu-coordination';

	// Constants
	const ANIMATION_SHIFT = '6px';

	interface Props<T = any> {
		testId?: string;
		children?: Snippet<[item: T]>;
		leftClickTrigger?: HTMLElement;
		rightClickTrigger?: HTMLElement;
		parentMenuId?: string;
		onclose?: () => void;
		onopen?: () => void;
		ontoggle?: (isOpen: boolean, isLeftClick: boolean) => void;
		onclick?: () => void;
		onkeypress?: () => void;
		menu?: Snippet<[{ close: () => void }]>;
		side?: 'top' | 'bottom' | 'left' | 'right';
		align?: 'start' | 'center' | 'end';
	}

	let {
		testId,
		leftClickTrigger,
		rightClickTrigger,
		parentMenuId,
		side = 'bottom',
		align = 'end',
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
	let isVisible = $state(false);
	let menuPosition = $state({ x: 0, y: 0 });

	// Generate unique menu ID
	let menuId = `menu-${Math.random().toString(36).substr(2, 9)}`;

	// Set up submenu coordination context
	const openSubmenus = new Set<() => void>();
	const submenuCoordination = {
		closeAll: () => {
			openSubmenus.forEach((close) => close());
		},
		register: (closeCallback: () => void) => {
			openSubmenus.add(closeCallback);
			return () => openSubmenus.delete(closeCallback);
		},
		hasOpenSubmenus: () => openSubmenus.size > 0,
		getMenuContainer: () => menuContainer,
		getMenuId: () => menuId,
		closeEntireMenu: () => {
			// Close this menu and all its children
			menuManager.closeMenu(menuId);
		}
	};

	setContext(SUBMENU_CONTEXT_KEY, submenuCoordination);

	// Cleanup on destroy
	onDestroy(() => {
		if (isVisible) {
			menuManager.unregister(menuId);
		}
	});

	function calculatePosition(
		target: HTMLElement | MouseEvent,
		alignOverride?: 'start' | 'center' | 'end'
	): { x: number; y: number } {
		const rect =
			target instanceof HTMLElement
				? target.getBoundingClientRect()
				: { x: target.clientX, y: target.clientY, width: 0, height: 0 };

		let x = rect.x;
		let y = rect.y;

		// Get menu dimensions for proper positioning
		const menuWidth = menuContainer?.offsetWidth || 0;
		const menuHeight = menuContainer?.offsetHeight || 0;
		const useAlign = alignOverride ?? align;

		// Position based on side
		if (side === 'top') {
			y = rect.y - menuHeight;
			// Adjust horizontal alignment for top/bottom
			if (useAlign === 'start') {
				x = rect.x;
			} else if (useAlign === 'center') {
				x = rect.x + rect.width / 2 - menuWidth / 2;
			} else if (useAlign === 'end') {
				x = rect.x + rect.width - menuWidth;
			}
		} else if (side === 'bottom') {
			y = rect.y + rect.height;
			// Adjust horizontal alignment for top/bottom
			if (useAlign === 'start') {
				x = rect.x;
			} else if (useAlign === 'center') {
				x = rect.x + rect.width / 2 - menuWidth / 2;
			} else if (useAlign === 'end') {
				x = rect.x + rect.width - menuWidth;
			}
		} else if (side === 'left') {
			x = rect.x - menuWidth;
			// Adjust vertical alignment for left/right
			if (useAlign === 'start') {
				y = rect.y;
			} else if (useAlign === 'center') {
				y = rect.y + rect.height / 2 - menuHeight / 2;
			} else if (useAlign === 'end') {
				y = rect.y + rect.height - menuHeight;
			}
		} else if (side === 'right') {
			x = rect.x + rect.width;
			// Adjust vertical alignment for left/right
			if (useAlign === 'start') {
				y = rect.y;
			} else if (useAlign === 'center') {
				y = rect.y + rect.height / 2 - menuHeight / 2;
			} else if (useAlign === 'end') {
				y = rect.y + rect.height - menuHeight;
			}
		}

		return { x, y };
	}

	function constrainToViewport(position: { x: number; y: number }): { x: number; y: number } {
		if (!menuContainer) return position;

		const menuWidth = menuContainer.offsetWidth;
		const menuHeight = menuContainer.offsetHeight;
		const viewport = {
			width: window.innerWidth,
			height: window.innerHeight
		};
		const MARGIN = 16; // Minimum margin from viewport edges

		let { x, y } = position;

		// Constrain horizontally
		const maxX = viewport.width - menuWidth - MARGIN;
		const minX = MARGIN;
		x = Math.max(minX, Math.min(x, maxX));

		// Constrain vertically
		const maxY = viewport.height - menuHeight - MARGIN;
		const minY = MARGIN;
		y = Math.max(minY, Math.min(y, maxY));

		return { x, y };
	}

	function executeByTrigger(callback: (isOpened: boolean, isLeftClick: boolean) => void) {
		const isLeftClick = Boolean(leftClickTrigger);
		const isRightClick = Boolean(rightClickTrigger);

		if (isLeftClick || isRightClick) {
			callback(isVisible, isLeftClick);
		}
	}

	function setPosition(e?: MouseEvent) {
		const isRightClick = Boolean(e && rightClickTrigger);
		const target = isRightClick ? e : leftClickTrigger;
		if (!target) return;

		// For right-click: try align 'start', then 'end', then 'center' if needed
		if (isRightClick && menuContainer) {
			let pos = calculatePosition(target, 'start');
			let constrained = constrainToViewport(pos);
			if (constrained.x !== pos.x || constrained.y !== pos.y) {
				// 'start' would go offscreen, try 'end'
				pos = calculatePosition(target, 'end');
				constrained = constrainToViewport(pos);
				if (constrained.x !== pos.x || constrained.y !== pos.y) {
					// 'end' would go offscreen, try 'center'
					pos = calculatePosition(target, 'center');
					constrained = constrainToViewport(pos);
				}
			}
			menuPosition = constrained;
		} else {
			// For left-click, use the align prop as before
			const basePosition = calculatePosition(target);
			menuPosition = menuContainer ? constrainToViewport(basePosition) : basePosition;
		}
	}

	// Recalculate position when menu dimensions become available
	$effect(() => {
		if (
			isVisible &&
			menuContainer &&
			menuContainer.offsetWidth > 0 &&
			menuContainer.offsetHeight > 0
		) {
			// Recalculate with proper dimensions
			const isRightClick = rightClickTrigger && savedMouseEvent;
			const target = isRightClick ? savedMouseEvent : leftClickTrigger;
			if (target) {
				if (isRightClick) {
					let pos = calculatePosition(target, 'start');
					let constrained = constrainToViewport(pos);
					if (constrained.x !== pos.x || constrained.y !== pos.y) {
						pos = calculatePosition(target, 'end');
						constrained = constrainToViewport(pos);
						if (constrained.x !== pos.x || constrained.y !== pos.y) {
							pos = calculatePosition(target, 'center');
							constrained = constrainToViewport(pos);
						}
					}
					menuPosition = constrained;
				} else {
					const basePosition = calculatePosition(target);
					menuPosition = constrainToViewport(basePosition);
				}
			}
		}
	});

	let savedMouseEvent: MouseEvent | undefined = $state();

	export function open(e?: MouseEvent, newItem?: T) {
		if (isVisible) return;

		// Save the mouse event for repositioning
		if (e) savedMouseEvent = e;

		// Calculate position first (before showing) using the triggering event
		setPosition(e);

		isVisible = true;
		if (newItem !== undefined) item = newItem;

		// Register with menu manager once the menu is rendered
		setTimeout(() => {
			if (menuContainer) {
				menuManager.register({
					id: menuId,
					element: menuContainer,
					parentMenuId,
					close: () => {
						isVisible = false;
						savedMouseEvent = undefined;
						onclose?.();
						if (ontoggle) executeByTrigger(ontoggle);
					}
				});
			}
		}, 0);

		onopen?.();
		if (ontoggle) executeByTrigger(ontoggle);
	}

	export function close() {
		if (!isVisible) return;

		// Unregister from menu manager
		menuManager.unregister(menuId);

		isVisible = false;
		savedMouseEvent = undefined;
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

	function getTransformOrigin(): string {
		// Calculate origin based on side and alignment
		const verticalOrigin =
			side === 'top'
				? 'bottom'
				: side === 'bottom'
					? 'top'
					: align === 'start'
						? 'top'
						: align === 'end'
							? 'bottom'
							: 'center';

		const horizontalOrigin =
			side === 'left'
				? 'right'
				: side === 'right'
					? 'left'
					: align === 'start'
						? 'left'
						: align === 'end'
							? 'right'
							: 'center';

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

	$effect(() => {
		if (!menuContainer) return;
		const config = { attributes: false, childList: true, subtree: true };

		const observer = new MutationObserver((mutationList) => {
			for (const mutation of mutationList) {
				if (mutation.type === 'childList') {
					if (isVisible && savedMouseEvent) {
						setPosition(savedMouseEvent);
					}
				}
			}
		});
		observer.observe(menuContainer, config);

		return () => observer.disconnect();
	});
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
				? ANIMATION_SHIFT
				: side === 'bottom'
					? `-${ANIMATION_SHIFT}`
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
		margin-top: -4px;
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
		max-height: calc(100vh - 16px); /* 8px margin top and bottom */
		overflow: hidden;
		overflow-y: auto;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		outline: none;
		background: var(--clr-bg-2);
		box-shadow: var(--fx-shadow-l);
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

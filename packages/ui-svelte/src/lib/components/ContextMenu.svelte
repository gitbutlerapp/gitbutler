<script lang="ts">
	import { focusable } from "$lib/focus/focusable";
	import { menuManager } from "$lib/utils/menuManager";
	import { portal } from "$lib/utils/portal";
	import { onMount, setContext, onDestroy, tick } from "svelte";
	import { type Snippet } from "svelte";

	// Context key for submenu coordination
	const SUBMENU_CONTEXT_KEY = "contextmenu-submenu-coordination";

	// Constants
	const ANIMATION_SHIFT = "6px";

	interface Props {
		testId?: string;
		children?: Snippet;
		leftClickTrigger?: HTMLElement;
		rightClickTrigger?: HTMLElement;
		parentMenuId?: string;
		onclose?: () => void;
		onopen?: () => void;
		onclick?: () => void;
		onkeypress?: () => void;
		side?: "top" | "bottom" | "left" | "right";
		align?: "start" | "center" | "end";
		/**
		 * Positioning target for the menu. The menu is positioned relative
		 * to this element or mouse event.
		 */
		target?: MouseEvent | HTMLElement;
	}

	let {
		testId,
		leftClickTrigger,
		rightClickTrigger,
		parentMenuId,
		side = "bottom",
		align = "end",
		children,
		onclose,
		onopen,
		onclick,
		onkeypress,
		target,
	}: Props = $props();

	let menuContainer: HTMLElement | undefined = $state();
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
		},
	};

	setContext(SUBMENU_CONTEXT_KEY, submenuCoordination);

	onDestroy(() => {
		menuManager.unregister(menuId);
	});

	onMount(async () => {
		// Save the target for repositioning after layout
		if (target instanceof MouseEvent || target instanceof PointerEvent) {
			savedMouseEvent = target;
		} else if (target instanceof HTMLElement) {
			savedElement = target;
		}

		await tick();

		if (target) setPosition(target);

		// Register with menu manager once the menu is rendered
		setTimeout(() => {
			if (menuContainer) {
				menuManager.register({
					id: menuId,
					element: menuContainer,
					triggerElement: leftClickTrigger ?? savedElement,
					parentMenuId,
					close: () => {
						onclose?.();
					},
				});
			}
		}, 0);

		onopen?.();
	});

	function calculatePosition(target: HTMLElement | MouseEvent): { x: number; y: number } {
		const rect =
			target instanceof HTMLElement
				? target.getBoundingClientRect()
				: { x: target.clientX, y: target.clientY, width: 0, height: 0 };

		let x = rect.x;
		let y = rect.y;

		// Get menu dimensions for proper positioning
		const menuWidth = menuContainer?.offsetWidth || 0;
		const menuHeight = menuContainer?.offsetHeight || 0;

		// Position based on side
		if (side === "top") {
			y = rect.y - menuHeight;
			// Adjust horizontal alignment for top/bottom
			if (align === "start") {
				x = rect.x;
			} else if (align === "center") {
				x = rect.x + rect.width / 2 - menuWidth / 2;
			} else if (align === "end") {
				x = rect.x + rect.width - menuWidth;
			}
		} else if (side === "bottom") {
			y = rect.y + rect.height;
			// Adjust horizontal alignment for top/bottom
			if (align === "start") {
				x = rect.x;
			} else if (align === "center") {
				x = rect.x + rect.width / 2 - menuWidth / 2;
			} else if (align === "end") {
				x = rect.x + rect.width - menuWidth;
			}
		} else if (side === "left") {
			x = rect.x - menuWidth;
			// Adjust vertical alignment for left/right
			if (align === "start") {
				y = rect.y;
			} else if (align === "center") {
				y = rect.y + rect.height / 2 - menuHeight / 2;
			} else if (align === "end") {
				y = rect.y + rect.height - menuHeight;
			}
		} else if (side === "right") {
			x = rect.x + rect.width;
			// Adjust vertical alignment for left/right
			if (align === "start") {
				y = rect.y;
			} else if (align === "center") {
				y = rect.y + rect.height / 2 - menuHeight / 2;
			} else if (align === "end") {
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
			height: window.innerHeight,
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

	function setPosition(target: MouseEvent | HTMLElement) {
		const basePosition = calculatePosition(target);
		menuPosition = menuContainer ? constrainToViewport(basePosition) : basePosition;
	}

	// Recalculate position when menu dimensions become available
	$effect(() => {
		if (menuContainer && menuContainer.offsetWidth > 0 && menuContainer.offsetHeight > 0) {
			const target = savedMouseEvent ?? savedElement ?? leftClickTrigger ?? rightClickTrigger;
			if (target) {
				setPosition(target);
			}
		}
	});

	let savedMouseEvent: MouseEvent | undefined = $state();
	let savedElement: HTMLElement | undefined = $state();

	function getTransformOrigin(): string {
		// Calculate origin based on side and alignment
		const verticalOrigin =
			side === "top"
				? "bottom"
				: side === "bottom"
					? "top"
					: align === "start"
						? "top"
						: align === "end"
							? "bottom"
							: "center";

		const horizontalOrigin =
			side === "left"
				? "right"
				: side === "right"
					? "left"
					: align === "start"
						? "left"
						: align === "end"
							? "right"
							: "center";

		return `${verticalOrigin} ${horizontalOrigin}`;
	}

	function handleKeyNavigation(e: KeyboardEvent) {
		if (e.key === "Escape") {
			e.preventDefault();
			onclose?.();
		}
	}

	// Close on any scroll event (use capture since scroll doesn't bubble).
	$effect(() => {
		function onScroll(e: Event) {
			// Don't close if the scroll is inside the menu itself.
			if (menuContainer?.contains(e.target as Node)) return;
			onclose?.();
		}

		document.addEventListener("scroll", onScroll, true);
		return () => document.removeEventListener("scroll", onScroll, true);
	});

	$effect(() => {
		if (!menuContainer) return;
		const config = { attributes: false, childList: true, subtree: true };

		const observer = new MutationObserver((mutationList) => {
			for (const mutation of mutationList) {
				if (mutation.type === "childList") {
					// Only reposition if we don't have open submenus
					// This prevents the menu from jumping when submenus open
					const target = savedMouseEvent ?? savedElement;
					if (target && !submenuCoordination.hasOpenSubmenus()) {
						setPosition(target);
					}
				}
			}
		});
		observer.observe(menuContainer, config);

		return () => observer.disconnect();
	});
</script>

<div class="portal-wrap" use:portal={"body"}>
	<!-- svelte-ignore a11y_autofocus -->
	<div
		data-testid={testId}
		bind:this={menuContainer}
		tabindex="-1"
		use:focusable={{ activate: true, isolate: true, focusable: true, dim: true, trap: true }}
		autofocus
		{onclick}
		{onkeypress}
		onkeydown={handleKeyNavigation}
		class="context-menu hide-native-scrollbar"
		class:top-oriented={side === "top"}
		class:bottom-oriented={side === "bottom"}
		class:left-oriented={side === "left"}
		class:right-oriented={side === "right"}
		style:top="{menuPosition.y}px"
		style:left="{menuPosition.x}px"
		style:transform-origin={getTransformOrigin()}
		style:--animation-transform-y-shift={side === "top"
			? ANIMATION_SHIFT
			: side === "bottom"
				? `-${ANIMATION_SHIFT}`
				: "0"}
		role="menu"
	>
		{@render children?.()}
	</div>
</div>

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
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		outline: none;
		background: var(--bg-2);
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

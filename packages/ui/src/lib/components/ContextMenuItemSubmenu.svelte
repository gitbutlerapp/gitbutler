<script lang="ts">
	import ContextMenu from '$components/ContextMenu.svelte';
	import ContextMenuItem from '$components/ContextMenuItem.svelte';
	import Icon from '$components/Icon.svelte';
	import { getContext, onDestroy } from 'svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	// Context key for submenu coordination
	const SUBMENU_CONTEXT_KEY = 'contextmenu-submenu-coordination';

	interface Props {
		icon?: keyof typeof iconsJson | undefined;
		label: string;
		disabled?: boolean;
		keyboardShortcut?: string;
		testId?: string;
		tooltip?: string;
		submenu: Snippet<[{ close: () => void }]>;
		submenuSide?: 'left' | 'right';
		submenuVerticalAlign?: 'top' | 'bottom';
	}

	const {
		icon,
		label,
		disabled = false,
		keyboardShortcut,
		testId,
		tooltip,
		submenu,
		submenuSide = 'right',
		submenuVerticalAlign = 'top'
	}: Props = $props();

	let menuItemElement: HTMLDivElement | undefined = $state();
	let contextMenu: ReturnType<typeof ContextMenu> | undefined = $state();
	let isSubmenuOpen = $state(false);
	let hoverTimeout: NodeJS.Timeout | undefined = $state();

	// Get submenu coordination context
	const submenuCoordination: {
		closeAll: () => void;
		register: (closeCallback: () => void) => () => void;
		hasOpenSubmenus: () => boolean;
		getMenuContainer: () => HTMLElement | undefined;
		getMenuId: () => string;
		closeEntireMenu: () => void;
	} = getContext(SUBMENU_CONTEXT_KEY) || {
		closeAll: () => {},
		register: () => () => {},
		hasOpenSubmenus: () => false,
		getMenuContainer: () => undefined,
		getMenuId: () => 'unknown',
		closeEntireMenu: () => {}
	};

	// Register this submenu
	const _unregister = submenuCoordination.register(() => {
		if (isSubmenuOpen) {
			closeSubmenu();
		}
	});

	// Cleanup on destroy
	onDestroy(() => {
		if (hoverTimeout) {
			clearTimeout(hoverTimeout);
		}
		_unregister();
	});

	function handleMouseEnter() {
		if (disabled) return;

		// Clear any existing timeout
		if (hoverTimeout) {
			clearTimeout(hoverTimeout);
		}

		// Small delay to prevent accidental opening
		hoverTimeout = setTimeout(() => {
			// Close all other submenus first
			submenuCoordination.closeAll();

			isSubmenuOpen = true;
			contextMenu?.open();
		}, 100);
	}

	function handleMouseLeave() {
		// Clear timeout if mouse leaves before delay completes
		if (hoverTimeout) {
			clearTimeout(hoverTimeout);
			hoverTimeout = undefined;
		}
	}

	function handleClick(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();

		if (disabled) return;

		isSubmenuOpen = !isSubmenuOpen;
		contextMenu?.toggle();
	}

	function closeSubmenu() {
		isSubmenuOpen = false;
		contextMenu?.close();
	}

	// Handle arrow key navigation
	function handleKeyDown(e: KeyboardEvent) {
		if (disabled) return;

		switch (e.key) {
			case 'ArrowRight':
				if (submenuSide === 'right') {
					e.preventDefault();
					e.stopPropagation();
					isSubmenuOpen = true;
					contextMenu?.open();
				}
				break;
			case 'ArrowLeft':
				if (submenuSide === 'left') {
					e.preventDefault();
					e.stopPropagation();
					isSubmenuOpen = true;
					contextMenu?.open();
				}
				break;
			case 'Enter':
			case ' ':
				e.preventDefault();
				e.stopPropagation();
				if (disabled) return;

				isSubmenuOpen = !isSubmenuOpen;
				contextMenu?.toggle();
				break;
		}
	}
</script>

<div
	bind:this={menuItemElement}
	class="submenu-wrapper"
	class:active={isSubmenuOpen}
	onmouseenter={handleMouseEnter}
	onmouseleave={handleMouseLeave}
	onkeydown={handleKeyDown}
	role="menuitem"
	aria-haspopup="menu"
	aria-expanded={isSubmenuOpen}
	tabindex="-1"
>
	<ContextMenuItem
		{icon}
		{label}
		{disabled}
		{keyboardShortcut}
		{testId}
		{tooltip}
		onclick={handleClick}
	>
		{#snippet control()}
			<div class="submenu-chevron">
				<Icon name="chevron-right-small" />
			</div>
		{/snippet}
	</ContextMenuItem>
</div>

<ContextMenu
	bind:this={contextMenu}
	leftClickTrigger={menuItemElement}
	parentMenuId={submenuCoordination.getMenuId()}
	side={submenuSide}
	align={submenuVerticalAlign === 'top' ? 'start' : 'end'}
	onclose={() => {
		isSubmenuOpen = false;
		menuItemElement?.focus();
	}}
>
	{@render submenu({ close: closeSubmenu })}
</ContextMenu>

<style lang="postcss">
	.submenu-wrapper {
		display: flex;
		position: relative;
		flex-direction: column;

		&.active :global(.menu-item) {
			background-color: var(--clr-bg-2-muted);
		}
	}

	.submenu-chevron {
		display: flex;
		flex-shrink: 0;
		transform: translateX(4px);
		color: var(--clr-text-3);
	}
</style>

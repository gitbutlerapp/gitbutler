<script lang="ts">
	import Button from '$components/Button.svelte';
	import ContextMenu from '$components/ContextMenu.svelte';
	import Icon from '$components/Icon.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		showOnHover?: boolean;
		minimal?: boolean;
		activated?: boolean;
		contextElement?: HTMLElement;
		testId?: string;
		el?: HTMLElement;
		contextMenu: Snippet<[{ close: () => void }]>;
		menuSide?: 'top' | 'bottom' | 'left' | 'right';
		menuAlign?: 'start' | 'center' | 'end';
		onMenuClose?: () => void;
		onMenuOpen?: () => void;
		onMenuToggle?: (isOpen: boolean, isLeftClick: boolean) => void;
	}

	let {
		showOnHover = false,
		minimal = false,
		activated = false,
		contextElement,
		testId,
		el = $bindable(),
		contextMenu: contextMenuSnippet,
		menuSide = 'bottom',
		menuAlign = 'end',
		onMenuClose,
		onMenuOpen,
		onMenuToggle
	}: Props = $props();

	let visible = $state(false);
	let buttonElement = $state<HTMLElement>();
	let internalContextMenu = $state<ReturnType<typeof ContextMenu>>();

	// Keep el in sync with buttonElement
	$effect(() => {
		el = buttonElement;
	});

	function onMouseEnter() {
		if (!showOnHover) return;
		visible = true;
	}

	function onMouseLeave() {
		if (!showOnHover) return;
		visible = false;
	}

	function onFocus() {
		if (!showOnHover) return;
		visible = true;
	}

	function onBlur() {
		if (!showOnHover) return;
		visible = false;
	}

	function onContextMenu(e: MouseEvent) {
		e.preventDefault(); // Prevent default to avoid browser context menu
		internalContextMenu?.open(e);
	}

	function onClick(e: MouseEvent) {
		e.stopPropagation();
		e.preventDefault();
		internalContextMenu?.toggle();
	}

	function closeMenu() {
		internalContextMenu?.close();
	}

	$effect(() => {
		if (contextElement) {
			contextElement.addEventListener('contextmenu', onContextMenu);
			contextElement.addEventListener('mouseenter', onMouseEnter);
			contextElement.addEventListener('mouseleave', onMouseLeave);
			contextElement.addEventListener('focus', onFocus);
			contextElement.addEventListener('blur', onBlur);
			return () => {
				contextElement.removeEventListener('contextmenu', onContextMenu);
				contextElement.removeEventListener('mouseenter', onMouseEnter);
				contextElement.removeEventListener('mouseleave', onMouseLeave);
				contextElement.removeEventListener('focus', onFocus);
				contextElement.removeEventListener('blur', onBlur);
			};
		}
	});
</script>

{#if showOnHover || minimal}
	<button
		bind:this={buttonElement}
		type="button"
		class="kebab-btn"
		class:visible
		class:activated
		class:show-on-hover={showOnHover}
		class:minimal
		onclick={onClick}
		oncontextmenu={onContextMenu}
		data-testid={testId}
	>
		<Icon name="kebab" />
	</button>
{:else}
	<Button
		bind:el={buttonElement}
		{testId}
		size="tag"
		icon="kebab"
		kind="ghost"
		{activated}
		onclick={onClick}
		oncontextmenu={onContextMenu}
	/>
{/if}

<ContextMenu
	bind:this={internalContextMenu}
	leftClickTrigger={buttonElement}
	rightClickTrigger={contextElement}
	side={menuSide}
	align={menuAlign}
	{testId}
	onclose={() => {
		onMenuClose?.();
	}}
	onopen={() => {
		onMenuOpen?.();
	}}
	ontoggle={(isOpen, isLeftClick) => {
		onMenuToggle?.(isOpen, isLeftClick);
	}}
>
	{@render contextMenuSnippet({ close: closeMenu })}
</ContextMenu>

<style lang="postcss">
	.kebab-btn {
		display: flex;
		padding: 0 3px;
		color: var(--clr-text-1);

		&.show-on-hover {
			display: none;

			&.visible {
				display: flex;
				opacity: 0.5;
			}

			&.activated,
			&:hover,
			&:focus-within {
				display: flex;
				opacity: 1;
			}
		}

		&.minimal {
			opacity: 0.7;

			&.activated,
			&:hover,
			&:focus-within {
				opacity: 1;
			}
		}
	}
</style>

<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import Tooltip from '$components/Tooltip.svelte';
	import { keysStringToArr } from '$lib/utils/hotkeys';
	import { getContext } from 'svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	// Context key for submenu coordination
	const SUBMENU_CONTEXT_KEY = 'contextmenu-submenu-coordination';

	interface Props {
		icon?: keyof typeof iconsJson | undefined;
		emoji?: string;
		label: string;
		disabled?: boolean;
		selected?: boolean;
		control?: Snippet;
		keyboardShortcut?: string;
		caption?: string;
		onclick: (e: MouseEvent) => void;
		testId?: string;
		tooltip?: string;
	}

	const {
		onclick,
		icon,
		emoji,
		label,
		disabled,
		selected = false,
		control,
		keyboardShortcut,
		caption,
		testId,
		tooltip
	}: Props = $props();

	// Get submenu coordination context if available
	const submenuCoordination:
		| {
				closeAll: () => void;
				hasOpenSubmenus: () => boolean;
				getMenuContainer: () => HTMLElement | undefined;
				getMenuId: () => string;
				closeEntireMenu: () => void;
		  }
		| undefined = getContext(SUBMENU_CONTEXT_KEY);

	function handleMouseEnter() {
		// Close any open submenus when hovering over a regular menu item
		submenuCoordination?.closeAll();
	}

	function handleClick(e: MouseEvent) {
		if (disabled) return;
		e.stopPropagation();
		onclick(e);
	}
</script>

{#snippet button()}
	<button
		data-testid={testId}
		type="button"
		class="menu-item focus-state no-select"
		style:--item-height={caption ? 'auto' : '1.625rem'}
		class:disabled
		{disabled}
		onclick={handleClick}
		onmouseenter={handleMouseEnter}
	>
		<div class="menu-item__content">
			{#if emoji}
				<div class="text-12">
					{emoji}
				</div>
			{:else if icon}
				<div class="menu-item__icon">
					<Icon name={icon} />
				</div>
			{/if}

			<span class="menu-item__label text-12">
				{label}
			</span>
			{#if keyboardShortcut}
				<span class="menu-item__shortcut text-12">
					{#each keysStringToArr(keyboardShortcut) as key}
						<span>{key}</span>
					{/each}
				</span>
			{/if}
			{#if control}
				{@render control()}
			{:else if selected}
				<div class="menu-item__icon">
					<Icon name="tick" />
				</div>
			{/if}
		</div>
		{#if caption}
			<div class="text-11 text-body menu-item__caption">
				{caption}
			</div>
		{/if}
	</button>
{/snippet}

{#if tooltip}
	<Tooltip text={tooltip}>
		{@render button()}
	</Tooltip>
{:else}
	{@render button()}
{/if}

<style lang="postcss">
	.menu-item {
		display: flex;
		flex-direction: column;
		justify-content: center;
		height: var(--item-height);
		padding: 6px 8px;
		gap: 4px;
		border-radius: var(--radius-s);
		color: var(--clr-text-1);
		text-align: left;
		cursor: pointer;
		transition: background-color var(--transition-fast);

		&:not(.disabled):hover {
			background-color: var(--clr-bg-2-muted);
			transition: none;
		}

		&.disabled {
			cursor: default;
			opacity: 0.3;
		}
	}

	.menu-item__content {
		display: flex;
		align-items: center;
		width: 100%;
		gap: 10px;
	}

	.menu-item__icon {
		display: flex;
		align-items: center;
		color: var(--clr-text-2);
	}

	.menu-item__label {
		flex-grow: 1;
		white-space: nowrap;
	}

	.menu-item__shortcut {
		display: flex;
		margin-left: 2px;
		color: var(--clr-text-3);
	}

	.menu-item__caption {
		max-width: 230px;
		color: var(--clr-text-2);
	}
</style>

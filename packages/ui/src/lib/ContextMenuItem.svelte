<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { keysStringToArr } from '$lib/utils/hotkeys';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	interface Props {
		icon?: keyof typeof iconsJson | undefined;
		label: string;
		disabled?: boolean;
		control?: Snippet;
		keyboardShortcut?: string;
		onclick: (e: MouseEvent) => void;
	}

	const { onclick, icon, label, disabled, control, keyboardShortcut }: Props = $props();
</script>

<button type="button" class="menu-item no-select" class:disabled {disabled} {onclick}>
	{#if icon}
		<Icon name={icon} />
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
	{/if}
</button>

<style lang="postcss">
	.menu-item {
		cursor: pointer;
		display: flex;
		text-align: left;
		align-items: center;
		color: var(--clr-text-1);
		padding: 6px 8px;
		border-radius: var(--radius-s);
		gap: 12px;
		transition: background-color var(--transition-fast);
		&:not(.disabled):hover {
			transition: none;
			background-color: var(--clr-bg-2-muted);
		}
		&:first-child {
			margin-top: 2px;
		}
		&:last-child {
			margin-bottom: 2px;
		}

		&.disabled {
			cursor: default;
			opacity: 0.3;
		}
	}

	.menu-item__label {
		flex-grow: 1;
		white-space: nowrap;
	}

	.menu-item__shortcut {
		display: flex;
		gap: 4px;
		color: var(--clr-text-3);
	}
</style>

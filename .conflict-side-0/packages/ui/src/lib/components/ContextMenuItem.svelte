<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import Tooltip from '$components/Tooltip.svelte';
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
		testId?: string;
		tooltip?: string;
	}

	const { onclick, icon, label, disabled, control, keyboardShortcut, testId, tooltip }: Props =
		$props();
</script>

{#snippet button()}
	<button
		data-testid={testId}
		type="button"
		class="menu-item focus-state no-select"
		class:disabled
		{disabled}
		{onclick}
	>
		{#if icon}
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
		align-items: center;
		padding: 6px 8px;
		gap: 10px;
		border-radius: var(--radius-s);
		color: var(--clr-text-1);
		text-align: left;
		cursor: pointer;
		transition: background-color var(--transition-fast);
		&:not(.disabled):hover {
			background-color: var(--clr-bg-2-muted);
			transition: none;
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
		gap: 4px;
		color: var(--clr-text-3);
	}
</style>

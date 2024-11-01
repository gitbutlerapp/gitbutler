<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	interface Props {
		icon?: keyof typeof iconsJson | undefined;
		label: string;
		disabled?: boolean;
		control?: Snippet;
		onclick: (e: MouseEvent) => void;
	}

	const { onclick, icon = undefined, label, disabled, control }: Props = $props();
</script>

<button type="button" class="menu-item" class:disabled {disabled} {onclick}>
	{#if icon}
		<Icon name={icon} />
	{/if}

	<span class="label text-12">
		{label}
	</span>
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
		color: var(--clr-scale-ntrl-0);
		padding: 6px 8px;
		border-radius: var(--radius-s);
		gap: 12px;
		transition: background-color var(--transition-fast);
		&:not(:global(.disabled)):hover {
			transition: none;
			background-color: var(--clr-bg-2-muted);
		}
		&:first-child {
			margin-top: 2px;
		}
		&:last-child {
			margin-bottom: 2px;
		}
	}
	.label {
		user-select: none;
		flex-grow: 1;
		white-space: nowrap;
	}
	.disabled {
		cursor: default;
		color: var(--clr-scale-ntrl-50);
	}
</style>

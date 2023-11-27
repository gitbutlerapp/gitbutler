<script lang="ts">
	import type iconsJson from '$lib/icons/icons.json';
	import Icon from '$lib/icons/Icon.svelte';
	import { getContext } from 'svelte';
	import type { ContextMenuContext } from './contextMenu';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let checked: boolean;
	const context = getContext<ContextMenuContext>('ContextMenu');
</script>

<button class="menu-item" on:click>
	{#if icon}
		<Icon name={icon} />
	{/if}
	{#if context.type == 'checklist'}
		<Icon name={checked ? 'tick-small' : 'empty'} />
	{/if}

	<div class="label">
		<slot />
	</div>
</button>

<style lang="postcss">
	.menu-item {
		display: flex;
		align-items: center;
		color: var(--clr-theme-scale-ntrl-0);
		padding: var(--space-4) var(--space-6);
		border-radius: var(--s);
		&:hover {
			background: var(--container-sub);
		}
		gap: var(--space-8);
	}
</style>

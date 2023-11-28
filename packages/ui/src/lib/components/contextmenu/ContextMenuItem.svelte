<script lang="ts">
	import type iconsJson from '$lib/icons/icons.json';
	import Icon from '$lib/icons/Icon.svelte';
	import { getContext } from 'svelte';
	import type { ContextMenuContext } from './contextMenu';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let checked = false;
	export let id: string | undefined = undefined;
	export let label: string;
	export let selected = false;
	export let disabled = false;

	const context = getContext<ContextMenuContext>('ContextMenu');
	const selection$ = context.selection$;

	$: if (selected && id) selection$.next({ id, label });
</script>

<button
	class="menu-item"
	class:disabled
	{id}
	on:click
	on:click={() => {
		if (id && !disabled) selection$.next({ id, label });
	}}
>
	{#if icon}
		<Icon name={icon} />
	{/if}
	{#if context.type == 'checklist'}
		<Icon name={checked ? 'tick-small' : 'empty'} />
	{/if}

	<div class="label">
		{label}
	</div>
</button>

<style lang="postcss">
	.menu-item {
		display: flex;
		align-items: center;
		color: var(--clr-theme-scale-ntrl-0);
		padding: var(--space-4) var(--space-6);
		border-radius: var(--radius-s);
		gap: var(--space-8);
		&:not(.disabled):hover {
			background: var(--clr-theme-container-sub);
		}
	}
	.label {
		white-space: nowrap;
	}
	.disabled {
		color: var(--clr-theme-scale-ntrl-50);
	}
</style>

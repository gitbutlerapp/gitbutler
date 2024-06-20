<script lang="ts">
	import Icon from '$lib/shared/Icon.svelte';
	import type iconsJson from '$lib/icons/icons.json';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let label: string;
	export let disabled = false;
</script>

<button class="menu-item" class:disabled {disabled} on:mousedown on:click>
	{#if icon}
		<Icon name={icon} />
	{/if}

	<span class="label text-base-12">
		{label}
	</span>
	<slot name="control" />
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

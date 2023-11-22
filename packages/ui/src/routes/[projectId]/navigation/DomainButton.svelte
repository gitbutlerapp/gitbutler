<script lang="ts">
	import { page } from '$app/stores';
	import Icon from '$lib/icons/Icon.svelte';
	import type iconsJson from '$lib/icons/icons.json';

	export let href: string;
	export let icon: keyof typeof iconsJson | undefined = undefined;
	$: selected = $page.url.href.includes(href);
</script>

<a
	{href}
	class="domain-button font-semibold"
	style:background-color={selected ? 'var(--clr-theme-container-pale)' : undefined}
>
	{#if icon}
		<Icon name={icon} class="text-color-4 mr-1 inline h-4 w-4 align-middle" />
	{/if}
	<slot />
</a>

<style lang="postcss">
	.domain-button {
		display: flex;
		align-items: center;
		gap: var(--space-10);
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-container-light);
		padding: var(--space-10);
		&:hover,
		&:focus {
			background-color: var(--clr-theme-container-pale);
		}
	}
</style>

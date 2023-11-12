<script lang="ts">
	import { page } from '$app/stores';
	import type { ComponentType } from 'svelte';

	export let href: string;
	export let icon: ComponentType | undefined = undefined;
	$: selected = $page.url.href.includes(href);
</script>

<a
	class="mx-4 block rounded px-3 py-2 font-semibold"
	class:bg-color-3={selected}
	{href}
	style:background-color={selected ? 'var(--bg-surface-highlight)' : undefined}
>
	<svelte:component this={icon} class="text-color-4 mr-1 inline h-4 w-4 align-middle" />
	<slot />
</a>

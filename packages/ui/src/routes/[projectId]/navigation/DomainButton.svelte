<script lang="ts">
	import { page } from '$app/stores';
	import type { ComponentType } from 'svelte';

	export let href: string;
	export let icon: ComponentType | undefined = undefined;
	$: selected = $page.url.href.includes(href);
</script>

<a
	{href}
	class="mx-4 flex items-center gap-x-1 rounded px-3 py-2 font-semibold"
	style:background-color={selected ? 'var(--bg-surface-highlight)' : undefined}
>
	<svelte:component this={icon} class="text-color-4 mr-1 inline h-4 w-4 align-middle" />
	<slot />
</a>

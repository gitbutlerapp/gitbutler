<script lang="ts">
	import { onMount } from 'svelte';
	import { IconExternalLink } from '../icons';
	import { open } from '@tauri-apps/api/shell';

	let classes = '';
	export { classes as class };
	export let target: '_blank' | '_self' | '_parent' | '_top' | undefined = undefined;
	export let rel: string | undefined = undefined;
	export let role: 'basic' | 'primary' | 'destructive' = 'basic';
	export let disabled = false;
	export let href: string | undefined = undefined;

	let element: HTMLAnchorElement | HTMLButtonElement | undefined;

	onMount(() => {
		if (element) {
			element.ariaLabel = element.innerText?.trim();
		}
	});

	$: isExternal = href?.startsWith('http');
</script>

{#if href}
	<a
		{href}
		{target}
		{rel}
		class="link inline-flex cursor-pointer items-center justify-center gap-1 whitespace-nowrap font-medium hover:underline hover:ease-in {role} {classes}"
		bind:this={element}
		class:disabled
		on:click={() => href && isExternal && open(href)}
	>
		<div class="truncate">
			<slot />
		</div>
		<div class="shrink-0">
			{#if isExternal}
				<IconExternalLink class="h-3.5 w-3.5" />
			{/if}
		</div>
	</a>
{/if}

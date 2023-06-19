<script lang="ts">
	import { onMount } from 'svelte';
	import { IconExternalLink } from '../../icons';

	export let target: '_blank' | '_self' | '_parent' | '_top' | undefined = undefined;
	export let rel: string | undefined = undefined;
	export let role: 'basic' | 'primary' | 'destructive' = 'basic';
	export let disabled = false;
	export let href: string | undefined = undefined;

	let element: HTMLAnchorElement | HTMLButtonElement;

	onMount(() => {
		element.ariaLabel = element.innerText?.trim();
	});

	$: isExternal = href?.startsWith('http');
</script>

<a {href} {target} {rel} class="link {role}" bind:this={element} class:disabled on:click>
	<slot />
	{#if isExternal}
		<IconExternalLink class="h-4 w-4" />
	{/if}
</a>

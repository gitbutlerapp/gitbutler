<script lang="ts">
	import { onMount } from 'svelte';
	import { IconExternalLink } from '../../icons';

	let classes = '';
	export { classes as class };
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

<a
	{href}
	{target}
	{rel}
	class="link flex items-center {role} {classes}"
	bind:this={element}
	class:disabled
	on:click
>
	<div class="flex-grow truncate">
		<slot />
	</div>
	<div class="shrink-0">
		{#if isExternal}
			<IconExternalLink class="h-4 w-4" />
		{/if}
	</div>
</a>

<script lang="ts">
	import { onMount } from 'svelte';
	import { IconExternalLink } from '../icons';

	export let target: '_blank' | '_self' | '_parent' | '_top' | undefined = undefined;
	export let rel: string | undefined = undefined;
	export let role: 'basic' | 'primary' | 'destructive' = 'basic';
	export let disabled = false;
	export let href: string | undefined = undefined;

	let element: HTMLAnchorElement | HTMLButtonElement;

	onMount(() => {
		element.ariaLabel = element.innerText.trim();
	});

	const isExternal = href?.startsWith('http');
</script>

<a {href} {target} {rel} class={role} bind:this={element} class:disabled>
	<slot />
	{#if isExternal}
		<IconExternalLink class="h-4 w-4 text-zinc-600" />
	{/if}
</a>

<style lang="postcss">
	a {
		@apply relative flex w-fit cursor-pointer items-center justify-center gap-[10px] whitespace-nowrap rounded text-base font-medium transition transition duration-150 ease-in-out ease-out hover:underline hover:ease-in;
		text-underline-offset: 3px;
	}

	a:focus {
		@apply outline-none;
	}

	.basic {
		@apply text-zinc-300;
	}

	.basic,
	.primary,
	.destructive {
		line-height: 20px;
	}

	.primary {
		@apply text-blue-500;
	}

	.destructive {
		@apply text-red-600;
	}

	.disabled {
		@apply pointer-events-none text-zinc-500;
	}
</style>

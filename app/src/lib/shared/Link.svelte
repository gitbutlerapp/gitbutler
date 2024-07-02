<script lang="ts">
	import Icon from '$lib/shared/Icon.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { onMount } from 'svelte';

	let classes = '';
	export { classes as class };
	export let target: '_blank' | '_self' | '_parent' | '_top' | undefined = undefined;
	export let rel: string | undefined = undefined;
	export let role: 'basic' | 'primary' | 'error' = 'basic';
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
		class="link {role} {classes}"
		bind:this={element}
		class:disabled
		on:click={(e) => {
			if (href && isExternal) {
				e.preventDefault();
				e.stopPropagation();
				openExternalUrl(href);
			}
		}}
	>
		<slot />
		{#if isExternal}
			<div class="link-icon">
				<Icon name="open-link" />
			</div>
		{/if}
	</a>
{/if}

<style lang="postcss">
	.link {
		cursor: pointer;
		display: inline-flex;
		align-items: center;
		gap: 2px;
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);
		text-decoration: underline;
		user-select: text;

		&:hover {
			text-decoration: none;
		}
	}
	.link-icon {
		flex-shrink: 0;
	}
</style>

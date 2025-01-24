<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import { getExternalLinkService } from '$lib/link/externalLinkService';
	import { onMount, type Snippet } from 'svelte';

	interface Props {
		href: string;
		children: Snippet;
		class?: string;
		target?: '_blank' | '_self' | '_parent' | '_top' | undefined;
		rel?: string | undefined;
		role?: 'basic' | 'primary' | 'error';
		disabled?: boolean;
	}

	const {
		href,
		target = undefined,
		class: classes,
		rel = undefined,
		role = 'basic',
		disabled = false,
		children
	}: Props = $props();

	let element = $state<HTMLAnchorElement | HTMLButtonElement>();

	const externalLinkService = getExternalLinkService();

	onMount(() => {
		if (element) {
			element.ariaLabel = element.innerText?.trim();
		}
	});

	const isExternal = $derived(href?.startsWith('http'));
</script>

<a
	{href}
	{target}
	{rel}
	class="link {role} {classes}"
	bind:this={element}
	class:disabled
	onclick={(e) => {
		if (href && isExternal) {
			e.preventDefault();
			e.stopPropagation();
			externalLinkService.open(href);
		}
	}}
>
	{@render children()}
	{#if isExternal}
		<div class="link-icon">
			<Icon name="open-link" />
		</div>
	{/if}
</a>

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
		opacity: 0.7;
	}
</style>

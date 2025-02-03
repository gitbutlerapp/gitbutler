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
		underline?: boolean;
		externalIcon?: boolean;
		disabled?: boolean;
	}

	const {
		href,
		target = undefined,
		class: classes,
		rel = undefined,
		role = 'basic',
		underline = true,
		externalIcon = true,
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
	class:underline
	onclick={(e) => {
		if (href && isExternal) {
			e.preventDefault();
			e.stopPropagation();
			externalLinkService.open(href);
		}
	}}
>
	{@render children()}
	{#if isExternal && externalIcon}
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
		user-select: text;
		text-decoration: none !important;

		&:hover {
			text-decoration: none;
		}
	}

	.link.underline {
		text-decoration: underline !important;
	}

	.link-icon {
		display: flex;
		flex-shrink: 0;
		opacity: 0.7;
	}
</style>

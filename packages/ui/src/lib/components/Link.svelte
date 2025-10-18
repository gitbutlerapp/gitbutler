<script lang="ts">
	import { onMount, type Snippet } from 'svelte';

	interface Props {
		href: string;
		children: Snippet;
		class?: string;
		target?: '_blank' | '_self' | '_parent' | '_top' | undefined;
		rel?: string | undefined;
	}

	const { href, target = undefined, class: classes, rel = undefined, children }: Props = $props();

	let element = $state<HTMLAnchorElement>();

	onMount(() => {
		if (element) {
			element.ariaLabel = element.innerText?.trim();
		}
	});

	const isExternal = $derived(href?.startsWith('http'));
</script>

<a {href} {target} {rel} class="link {classes}" bind:this={element}>
	<span class="underline">
		{@render children()}
	</span>

	{#if isExternal}
		<span class="link-icon">↗</span>
	{/if}
</a>

<style lang="postcss">
	.link {
		display: inline;
		align-items: center;
		text-decoration: none;
		cursor: pointer;
		transition: background-color var(--transition-fast);

		&:hover .link-icon {
			opacity: 1;
		}
	}

	.underline:hover {
		text-decoration: none;
	}

	.link-icon {
		opacity: 0.7;
		transition: opacity var(--transition-fast);
	}
</style>

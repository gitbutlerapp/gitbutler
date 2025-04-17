<script lang="ts">
	import Link from '$lib/link/Link.svelte';
	import type { Snippet } from 'svelte';

	type Props = { href: string; children: Snippet };
	const { href, children }: Props = $props();
</script>

<Link {href} target="_blank" rel="noopener noreferrer" bypassExternalLinkService>
	{@render children()}
</Link>

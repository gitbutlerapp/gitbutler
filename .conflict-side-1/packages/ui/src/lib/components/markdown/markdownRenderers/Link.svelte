<script lang="ts">
	import Link from '$components/Link.svelte';
	import type { Snippet } from 'svelte';

	type Props = { href: string; children: Snippet };
	const { href, children }: Props = $props();
</script>

<Link {href}>
	{@render children()}
</Link>

<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();
</script>

<blockquote>
	{@render children()}
</blockquote>

<style>
	blockquote {
		border-left: 2px solid #ccc;
		padding: 0 0 0 1rem;
	}
</style>

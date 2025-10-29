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
		padding: 0 0 0 1rem;
		border-left: 2px solid #ccc;
	}
</style>

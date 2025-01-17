<script lang="ts">
	import type { LayoutData } from './$types';
	import type { Snippet } from 'svelte';

	const { children }: { data: LayoutData; children: Snippet } = $props();
</script>

{@render children()}

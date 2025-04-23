<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		text: string;
		children: Snippet;
	}

	const { children }: Props = $props();
</script>

<p>
	{@render children()}
</p>

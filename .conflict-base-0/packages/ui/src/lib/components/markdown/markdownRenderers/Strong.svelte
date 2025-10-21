<script lang="ts">
	import { type Snippet } from 'svelte';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();
</script>

<span class="text-bold">
	{@render children()}
</span>

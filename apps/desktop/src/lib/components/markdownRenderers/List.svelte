<script lang="ts">
	import { type Snippet } from 'svelte';

	interface Props {
		ordered: string;
		start: number;
		children: Snippet;
	}

	const { ordered, start, children }: Props = $props();
</script>

{#if ordered}
	<ol {start}>
		{@render children()}
	</ol>
{:else}
	<ul>
		{@render children()}
	</ul>
{/if}

<script lang="ts">
	import { type Snippet } from 'svelte';

	interface Props {
		ordered: boolean;
		start: number | '';
		children: Snippet;
	}

	const { ordered, start, children }: Props = $props();
</script>

{#if ordered}
	<ol start={Number(start)}>
		{@render children()}
	</ol>
{:else}
	<ul>
		{@render children()}
	</ul>
{/if}

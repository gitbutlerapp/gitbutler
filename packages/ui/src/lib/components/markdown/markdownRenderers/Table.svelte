<script lang="ts">
	import { type Snippet } from "svelte";

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();
</script>

<table>
	{@render children()}
</table>

<style>
	table {
		width: 100%;
		margin: 1rem 0;
		border-collapse: collapse;
	}

	table :global(th),
	table :global(td) {
		padding: 0.5rem;
		border: 1px solid var(--clr-border-2);
		text-align: left;
	}

	table :global(th) {
		background-color: var(--clr-bg-2);
		font-weight: 600;
	}
</style>

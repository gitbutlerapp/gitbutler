<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		testId?: string;
		children: Snippet;
	}

	let { testId, children }: Props = $props();
</script>

<div class="card-group" data-testid={testId}>
	{@render children()}
</div>

<style lang="postcss">
	.card-group {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}
</style>

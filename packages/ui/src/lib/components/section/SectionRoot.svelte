<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		testId?: string;
		children: Snippet;
	}

	let { testId, children }: Props = $props();
</script>

<section class="section" data-testid={testId}>
	{@render children()}
</section>

<style lang="postcss">
	.section {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}
</style>

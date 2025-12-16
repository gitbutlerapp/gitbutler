<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		children?: Snippet;
	}

	const { children }: Props = $props();
</script>

<div class="disclaimer text-12 text-body">
	{@render children?.()}
</div>

<style>
	.disclaimer {
		padding: 10px 12px;
		border-radius: var(--radius-m);
		background: var(--clr-bg-2);
		color: var(--clr-text-2);
	}
</style>

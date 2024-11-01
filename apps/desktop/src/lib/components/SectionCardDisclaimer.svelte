<script lang="ts">
	import { type Snippet } from 'svelte';

	interface Props {
		children: Snippet;
	}

	let { children }: Props = $props();
</script>

<div class="disclaimer text-12 text-body">
	{@render children?.()}
</div>

<style>
	.disclaimer {
		color: var(--clr-scale-ntrl-50);
		background: var(--clr-bg-2);
		border-radius: var(--radius-m);
		padding: 10px 12px;
	}
</style>

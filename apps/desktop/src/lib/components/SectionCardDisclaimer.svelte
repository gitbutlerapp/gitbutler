<script lang="ts">
	interface Props {
		children?: import('svelte').Snippet;
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

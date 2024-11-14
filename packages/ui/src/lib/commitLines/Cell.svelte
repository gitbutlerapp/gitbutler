<script lang="ts">
	interface Props {
		isBottom?: boolean;
	}

	const { isBottom = false }: Props = $props();
</script>

<div class="commit-line stacked" class:dashed={isBottom}></div>

<style lang="postcss">
	.commit-line {
		margin: 0 22px 0 20px;
	}
</style>

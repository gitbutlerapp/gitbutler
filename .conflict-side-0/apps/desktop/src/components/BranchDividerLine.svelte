<script lang="ts">
	interface Props {
		lineColor: string;
		height?: string;
	}

	const { lineColor, height = '0.625rem' }: Props = $props();
</script>

<div class="commit-line" style:--commit-color={lineColor} style:height></div>

<style>
	.commit-line {
		margin: 0 21px;
		background-color: var(--commit-color);
	}
</style>

<script lang="ts">
	interface Props {
		lineColor: string;
	}

	const { lineColor }: Props = $props();
</script>

<div class="commit-line" style:--commit-color={lineColor}></div>

<style>
	.commit-line {
		height: 10px;
		margin: 0 21px;
		background-color: var(--commit-color);
	}
</style>

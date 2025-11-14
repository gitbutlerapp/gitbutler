<script lang="ts">
	interface Props {
		lineColor: string;
		short?: boolean;
	}

	const { lineColor, short = false }: Props = $props();
</script>

<div class="commit-line" class:short style:--commit-color={lineColor}></div>

<style>
	.commit-line {
		height: 12px;
		margin: 0 21px;
		background-color: var(--commit-color);

		&.short {
			height: 6px;
		}
	}
</style>

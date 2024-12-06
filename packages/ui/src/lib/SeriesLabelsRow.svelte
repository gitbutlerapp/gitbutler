<script lang="ts">
	import SeriesIcon from './SeriesIcon.svelte';
	import Tooltip from '$lib/Tooltip.svelte';

	interface Props {
		series: string[];
		showRestAmount?: boolean;
		selected?: boolean;
	}

	const { series, selected, showRestAmount }: Props = $props();
</script>

<div class="series-labels-row">
	<SeriesIcon single={series.length > 1} outlined={selected} />

	<div class="series-name text-12 text-semibold contrast">
		<span class="truncate">{series[0]}</span>
	</div>

	{#if series.length > 1}
		<svg
			class="more-series-arrow"
			width="14"
			height="12"
			viewBox="0 0 14 12"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path d="M2 6H12M12 6L6.6 1M12 6L6.6 11" stroke-width="1.5" />
		</svg>
	{/if}

	{#if showRestAmount && series.length > 1}
		<Tooltip text={'→ ' + series.slice(1).join(' → ')}>
			<div class="series-name more-series text-12 text-semibold">
				<span>{series.length - 1} more</span>
			</div>
		</Tooltip>
	{/if}
</div>

<style lang="postcss">
	.series-labels-row {
		display: flex;
		align-items: center;
		gap: 4px;
		width: fit-content;
		max-width: 100%;
		overflow: hidden;
	}

	.series-name {
		display: flex;
		align-items: center;
		color: var(--clr-text-2);
		margin-left: 3px;
		overflow: hidden;

		&.contrast {
			color: var(--clr-text-1);
		}
	}

	.more-series {
		flex-shrink: 0;
		max-width: fit-content;
	}

	.more-series-arrow {
		stroke: var(--clr-text-2);
		opacity: 0.6;
	}
</style>

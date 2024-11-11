<script lang="ts">
	import SeriesIcon from './SeriesIcon.svelte';
	import Tooltip from '$lib/Tooltip.svelte';

	interface Props {
		series: string[];
		showCounterLabel?: boolean;
		selected?: boolean;
	}

	const { series, selected, showCounterLabel }: Props = $props();
</script>

<div class="series-labels-row">
	<SeriesIcon single={series.length > 1} outlined={selected} />

	<div class="series-name text-12 text-semibold contrast">
		<span class="truncate">{series[0]}</span>
	</div>

	{#if showCounterLabel && series.length > 1}
		<Tooltip text={'🠶 ' + series.slice(1).join(' 🠶 ')}>
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
		height: 22px;
		padding: 2px 6px;
		background-color: oklch(from var(--clr-core-ntrl-60) l c h / 0.15);
		border-radius: var(--radius-m);
		/* width: 100%; */
		overflow: hidden;

		&.contrast {
			color: var(--clr-text-1);
		}
	}

	.more-series {
		flex-shrink: 0;
		max-width: fit-content;
	}
</style>

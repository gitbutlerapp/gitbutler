<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import Tooltip from '$lib/Tooltip.svelte';

	interface Props {
		series: string[];
		showCounterLabel?: boolean;
		selected?: boolean;
	}

	const { series, selected, showCounterLabel }: Props = $props();
</script>

<div class="series-labels-row">
	<Tooltip text={series.length > 1 ? 'Multiple branches' : 'Single branch'}>
		<div class="stack-icon" class:selected>
			<Icon name={series.length > 1 ? 'chain-link' : 'branch-small'} />
		</div>
	</Tooltip>

	<div class="series-name text-12 text-semibold contrast" class:selected>
		<span class="truncate">{series[0]}</span>
	</div>

	{#if showCounterLabel && series.length > 1}
		<div class="series-name text-12 text-semibold" class:selected>
			<span class="truncate">{series.length - 1} more</span>
		</div>
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

	.stack-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		width: 20px;
		height: 22px;
		color: var(--clr-text-2);
		background-color: var(--clr-theme-ntrl-soft);
		border-radius: var(--radius-m);

		&.selected {
			background-color: var(--clr-theme-ntrl-soft-hover);
		}
	}

	.series-name {
		display: flex;
		align-items: center;
		color: var(--clr-text-2);
		height: 22px;
		padding: 2px 6px;
		background-color: var(--clr-theme-ntrl-soft);
		border-radius: var(--radius-m);
		width: 100%;
		overflow: hidden;
		max-width: fit-content;

		&.contrast {
			color: var(--clr-text-1);
		}

		&.selected {
			background-color: var(--clr-theme-ntrl-soft-hover);
		}
	}
</style>

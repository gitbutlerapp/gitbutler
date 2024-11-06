<script lang="ts">
	import Icon from '$lib/Icon.svelte';

	interface Props {
		series: string[];
		showCounterLabel?: boolean;
	}

	const { series, showCounterLabel }: Props = $props();
</script>

<div class="series-labels-row">
	<div class="stack-icon">
		<Icon name={series.length > 1 ? 'chain-link' : 'branch-small'} />
	</div>

	<div class="series-name text-12 text-semibold" class:contrast={showCounterLabel}>
		<span class="truncate">{series[0]}</span>
	</div>

	{#if showCounterLabel}
		<div class="series-name text-12 text-semibold">
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
	}
</style>

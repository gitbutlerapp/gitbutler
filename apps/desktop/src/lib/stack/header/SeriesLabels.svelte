<script lang="ts">
	import Select from '$lib/select/Select.svelte';
	import { PatchSeries } from '$lib/vbranches/types';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import SeriesLabelsRow from '@gitbutler/ui/SeriesLabelsRow.svelte';

	interface Props {
		series: PatchSeries[];
		disableSelector?: boolean;
	}

	const { series, disableSelector }: Props = $props();

	let shiftedSeries = series.slice(1);
	let seriesTypes = shiftedSeries.map((s) => (s.patches[0] ? s.patches[0].status : 'local'));
</script>

<div class="stack-series-row">
	<SeriesLabelsRow series={series.map((s) => s.name)} />
	<!-- Selector -->
	{#if disableSelector}
		<div class="series-name text-12 text-semibold">
			<span class="truncate">{series.length} more</span>
		</div>
	{:else if series.length > 1}
		<div class="other-series">
			<Select
				popupAlign="right"
				customWidth={300}
				options={shiftedSeries.map((b) => ({ label: b.name, value: b.name }))}
				onselect={(value) => {
					// find in DOM and scroll to
					const el = document.querySelector(`[data-series-name="${value}"]`);

					if (!el) return;

					el.scrollIntoView({ behavior: 'smooth', block: 'start', inline: 'nearest' });
					el.classList.add('series-highlight-animation');

					setTimeout(() => {
						el.classList.remove('series-highlight-animation');
					}, 1000);
				}}
			>
				{#snippet customSelectButton()}
					<div class="other-series-select">
						<span class="text-12 text-semibold">{shiftedSeries.length} more</span>
						<Icon name="chevron-down-small" />
					</div>
				{/snippet}

				{#snippet itemSnippet({ item, idx })}
					<button type="button" class="other-series-item">
						<div class="other-series-chain-icon"></div>
						<div class="other-series-icon-and-name">
							<div class="other-series-icon {seriesTypes[idx]}"></div>
							<span class="other-series-name text-12 truncate">{item.label}</span>
						</div>
						<div class="other-series-scroll-to text-11">
							<span>Scroll here</span>
						</div>
					</button>
				{/snippet}
			</Select>
		</div>
	{/if}
</div>

<style lang="postcss">
	.stack-series-row {
		display: flex;
		align-items: center;
		gap: 4px;
		width: 100%;
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
	}

	/* SERIES SELECTOR */
	.other-series {
		position: relative;
		display: flex;
	}

	.other-series-select {
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 2px 4px 2px 6px;
		color: var(--clr-text-2);
		background-color: var(--clr-bg-1);
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		height: 100%;
		text-wrap: nowrap;
		transition: border-color var(--transition-fast);

		&:hover {
			border-color: var(--clr-border-1);
		}
	}

	.other-series-item {
		position: relative;
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		padding: 6px 2px 6px 6px;
		border-radius: var(--radius-s);

		&:hover {
			/* background-color: var(--clr-bg-1-muted); */

			.other-series-scroll-to {
				opacity: 1;
				flex: none;
			}
		}
	}

	.other-series-icon-and-name {
		display: flex;
		flex: 1;
		align-items: center;
		overflow: hidden;
	}

	.other-series-icon {
		position: relative;

		z-index: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		width: 8px;
		height: 8px;
		border-radius: 100%;
		color: var(--clr-core-ntrl-100);
		margin-right: 12px;

		&.local {
			background-color: var(--clr-commit-local);
		}

		&.localAndRemote {
			background-color: var(--clr-commit-remote);
		}

		&.integrated {
			background-color: var(--clr-commit-integrated);
		}
	}

	.other-series-chain-icon {
		pointer-events: none;
		position: absolute;
		top: -5px;
		left: 9px;
		width: 2px;
		height: 8px;
		background-color: var(--clr-border-3);
	}

	.other-series-name {
		color: var(--clr-text-1);
		width: 100%;
		text-align: left;
	}

	.other-series-scroll-to {
		overflow: hidden;
		flex: 0;
		width: fit-content;
		opacity: 0;
		flex-shrink: 1;
		text-wrap: nowrap;
		padding: 2px 4px;
		border-radius: var(--radius-s);
		color: var(--clr-text-2);
		background-color: var(--clr-theme-ntrl-soft);
	}
</style>

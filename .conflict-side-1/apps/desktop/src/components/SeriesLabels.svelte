<script lang="ts">
	import { isPatchSeries } from '$lib/branches/branch';
	import { PatchSeries } from '$lib/branches/branch';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import SeriesLabelsRow from '@gitbutler/ui/SeriesLabelsRow.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import { isError } from '@gitbutler/ui/utils/typeguards';

	interface Props {
		series: (PatchSeries | Error)[];
	}

	const { series }: Props = $props();

	const shiftedSeries = $derived(series.slice(1));
	const seriesTypes = $derived(
		shiftedSeries.map((s) => {
			if (isPatchSeries(s) && s.patches?.[0]) {
				return s.patches?.[0].status;
			}
			if (isError(s)) {
				return 'error';
			}
			return 'local';
		})
	);

	let selectorShown = $state(false);
</script>

<div class="stack-series-row">
	<SeriesLabelsRow series={series.map((s) => s.name)} />

	<!-- SERIES SELECTOR -->
	{#if series.length > 1}
		<div class="selector-series">
			<Select
				popupAlign="right"
				customWidth={300}
				options={shiftedSeries.map((b) => ({ label: b.name, value: b.name }))}
				ontoggle={(isOpen) => {
					selectorShown = isOpen;
				}}
				onselect={(value) => {
					// find in DOM and scroll to
					const el = document.querySelector(`[data-series-name="${value}"]`) as HTMLElement;

					if (!el) return;

					el.scrollIntoView({ behavior: 'smooth', block: 'start', inline: 'nearest' });

					setTimeout(() => {
						el.classList.add('series-highlight-animation');
					}, 300);

					setTimeout(() => {
						el.classList.remove('series-highlight-animation');
					}, 1200);
				}}
			>
				{#snippet customSelectButton()}
					<div class="selector-series-select" class:opened={selectorShown}>
						<span class="text-12 text-semibold">{shiftedSeries.length} more</span>
						<div class="selector-series-select__icon"><Icon name="chevron-down-small" /></div>
					</div>
				{/snippet}

				{#snippet itemSnippet({ item, idx })}
					<button type="button" class="selector-series-item">
						<div class="selector-series-chain-icon"></div>
						<div class="selector-series-icon-and-name">
							<div class="selector-series-icon {seriesTypes[idx]}"></div>
							<span class="selector-series-name text-12 text-semibold truncate">{item.label}</span>
						</div>
						<div class="selector-series-scroll-to text-11">
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
		width: 100%;
		overflow: hidden;
		gap: 4px;
	}

	.stack-icon {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 22px;
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-ntrl-soft);
		color: var(--clr-text-2);
	}

	.series-name {
		display: flex;
		align-items: center;
		width: 100%;
		max-width: fit-content;
		height: 22px;
		padding: 2px 6px;
		overflow: hidden;
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-ntrl-soft);
		color: var(--clr-text-2);
	}

	/* SERIES SELECTOR */
	.selector-series {
		display: flex;
		position: relative;
	}

	.selector-series-select {
		display: flex;
		align-items: center;
		margin-left: -2px;
		padding: 2px 4px 2px 6px;
		gap: 2px;
		border-radius: var(--radius-m);
		color: var(--clr-text-1);
		text-wrap: nowrap;
		transition: border-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}

		&.opened {
			background-color: var(--clr-bg-1-muted);

			& .selector-series-select__icon {
				transform: rotate(180deg);
			}
		}
	}

	.selector-series-select__icon {
		display: flex;
		color: var(--clr-text-2);
	}

	.selector-series-item {
		display: flex;
		position: relative;
		align-items: center;
		width: 100%;
		padding: 6px 2px 6px 6px;
		gap: 10px;
		border-radius: var(--radius-s);

		&:hover {
			.selector-series-scroll-to {
				flex: none;
				opacity: 1;
			}
		}
	}

	.selector-series-icon-and-name {
		display: flex;
		flex: 1;
		align-items: center;
		overflow: hidden;
	}

	.selector-series-icon {
		display: flex;

		z-index: 1;
		position: relative;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 8px;
		height: 8px;
		margin-right: 12px;
		border-radius: 100%;
		color: var(--clr-core-ntrl-100);

		&.local {
			background-color: var(--clr-commit-local);
		}

		&.localAndRemote {
			background-color: var(--clr-commit-remote);
		}

		&.integrated {
			background-color: var(--clr-commit-integrated);
		}

		&.error {
			background-color: var(--clr-theme-err-element);
		}
	}

	.selector-series-chain-icon {
		position: absolute;
		top: -5px;
		left: 9px;
		width: 2px;
		height: 8px;
		background-color: var(--clr-border-3);
		pointer-events: none;
	}

	.selector-series-name {
		width: 100%;
		color: var(--clr-text-1);
		text-align: left;
	}

	.selector-series-scroll-to {
		flex: 0;
		flex-shrink: 1;
		width: fit-content;
		padding: 2px 4px;
		overflow: hidden;
		border-radius: var(--radius-s);
		background-color: var(--clr-theme-ntrl-soft);
		color: var(--clr-text-2);
		text-wrap: nowrap;
		opacity: 0;
	}
</style>

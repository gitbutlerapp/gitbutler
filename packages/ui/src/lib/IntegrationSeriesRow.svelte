<script lang="ts" module>
	import type { Snippet } from 'svelte';
	export interface Props {
		type: 'clear' | 'conflicted' | 'integrated';
		series: string[];
		select: Snippet;
	}
</script>

<script lang="ts">
	import SeriesLabelsRow from './SeriesLabelsRow.svelte';
	import Icon from '$lib/Icon.svelte';

	let { type, series, select }: Props = $props();
</script>

<div class="integration-series-item no-select {type}">
	<div class="name-label-wrap">
		<SeriesLabelsRow {series} showCounterLabel selected={type === 'integrated'} />

		{#if type !== 'clear'}
			<span class="name-label-badge text-11 text-semibold">
				{#if type === 'conflicted'}
					<span>Conflicted</span>
				{:else if type === 'integrated'}
					<span>Integrated</span>
				{/if}
			</span>
		{/if}
	</div>

	{#if select}
		<div class="select">
			{@render select()}
		</div>
	{/if}

	{#if type === 'integrated'}
		<div class="integrated-label-wrap">
			<Icon name="tick-small" />
			<span class="integrated-label text-12"> Part of the new base </span>
		</div>
	{/if}
</div>

<style lang="postcss">
	.integration-series-item {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 12px 12px 14px;
		min-height: 56px;
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}

		.branch-icon {
			display: flex;
			align-items: center;
			justify-content: center;
			width: 16px;
			height: 16px;
			border-radius: var(--radius-s);
			color: var(--clr-core-ntrl-100);
		}

		/* NAME LABEL */
		.name-label-wrap {
			flex: 1;
			display: flex;
			align-items: center;
			gap: 8px;
			overflow: hidden;
		}

		.name-label-badge {
			padding: 4px 6px 3px;
			height: 100%;
			border-radius: var(--radius-m);
			color: var(--clr-core-ntrl-100);
		}

		/* INTEGRATED LABEL */
		.integrated-label-wrap {
			display: flex;
			align-items: center;
			gap: 4px;
			padding-left: 6px;
			margin-right: 2px;
			color: var(--clr-text-2);
		}

		.integrated-label {
			white-space: nowrap;
		}

		.select {
			max-width: 130px;
		}

		/* MODIFIERS */
		&.clear {
			background-color: var(--clr-bg-1);
		}

		&.conflicted {
			background-color: var(--clr-bg-1);

			.name-label-badge {
				background-color: var(--clr-theme-warn-on-element);
				background-color: var(--clr-theme-warn-element);
			}
		}

		&.integrated {
			background-color: var(--clr-bg-1-muted);

			.name-label-badge {
				color: var(--clr-theme-purp-on-element);
				background-color: var(--clr-theme-purp-element);
			}
		}
	}
</style>

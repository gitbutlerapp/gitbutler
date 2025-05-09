<script lang="ts" module>
	import type { Snippet } from 'svelte';

	type BranchStatus = 'integrated' | 'conflicted' | 'clear' | undefined;

	type Branch = {
		name: string;
		status: BranchStatus;
	};

	export type BranchShouldBeDeletedMap = {
		[branchName: string]: boolean;
	};

	export interface Props {
		testId?: string;
		series: Branch[];
		branchShouldBeDeletedMap: BranchShouldBeDeletedMap;
		updateBranchShouldBeDeletedMap: (branchName: string[], shouldBeDeleted: boolean) => void;
		children?: Snippet;
	}
</script>

<script lang="ts">
	import Checkbox from '$lib/Checkbox.svelte';
	import Icon from '$lib/Icon.svelte';
	import SeriesIcon from '$lib/SeriesIcon.svelte';
	const {
		testId,
		series,
		children,
		updateBranchShouldBeDeletedMap,
		branchShouldBeDeletedMap
	}: Props = $props();

	const allSeriesAreIntegrated = series.every((branch) => branch.status === 'integrated');
</script>

{#snippet stackBranch({ name, status }: Branch, isLast: boolean)}
	<div class="series-branch {status}">
		<div class="structure-lines" class:last={isLast}></div>
		<div class="branch-info">
			<span class="text-12 text-semibold truncate">{name}</span>

			{#if status}
				<span class="status-badge text-10 text-semibold">
					{#if status === 'conflicted'}
						Conflicted
					{:else if status === 'integrated'}
						Integrated
					{/if}
				</span>
			{/if}
		</div>

		{#if status === 'integrated'}
			<div class="integrated-label-wrap">
				<Icon name="tick-small" />
				<span class="integrated-label text-12"> Part of the new base </span>
			</div>
		{/if}
	</div>
{/snippet}

<div data-testid={testId} class="integration-series-item no-select">
	{#if series.length > 1}
		<div class="series-header" class:integrated={allSeriesAreIntegrated}>
			<div class="series-header-row">
				<div class="name-label-wrap">
					<SeriesIcon single={false} outlined />

					<span class="series-label text-12 text-semibold truncate"> Stack branches </span>
				</div>

				{#if allSeriesAreIntegrated}
					{@const atLeastSomeWillBeDeleted = series.some(
						(branch) => branchShouldBeDeletedMap[branch.name]
					)}
					<div class="integrated-label-wrap">
						<span class="integrated-label text-12">Delete all local branches</span>
						<Checkbox
							checked={atLeastSomeWillBeDeleted}
							onchange={(e) => {
								const shouldBeDeleted = e.currentTarget.checked;
								updateBranchShouldBeDeletedMap(
									series.map((branch) => branch.name),
									shouldBeDeleted
								);
							}}
						/>
					</div>
				{/if}
			</div>

			{#if children}
				{@render children()}
			{/if}
		</div>

		<div class="series-branches">
			{#each series as seriesItem, idx}
				{@render stackBranch(seriesItem, idx === series.length - 1)}
			{/each}
		</div>
	{:else if series.length === 1}
		{@const branch = series[0]}
		<div class="series-header {branch.status}">
			<div class="name-label-wrap">
				<SeriesIcon single={true} outlined />

				<span class="text-12 text-semibold truncate">
					{branch.name}
				</span>

				{#if branch.status}
					<div class="branch-status-info">
						<span class="status-badge text-10 text-semibold">
							{#if branch.status === 'conflicted'}
								Conflicted
							{:else if branch.status === 'integrated'}
								Integrated
							{/if}
						</span>

						{#if branch.status === 'integrated'}
							<div class="integrated-label-wrap">
								<span class="integrated-label text-12">Delete local branch</span>
								<Checkbox
									checked={branchShouldBeDeletedMap[branch.name]}
									onchange={(e) => {
										const shouldBeDeleted = e.currentTarget.checked;
										updateBranchShouldBeDeletedMap([branch.name], shouldBeDeleted);
									}}
								/>
							</div>
						{/if}
					</div>
				{/if}
			</div>

			{#if children}
				{@render children()}
			{/if}
		</div>
	{/if}
</div>

<style lang="postcss">
	.integration-series-item {
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}

		.series-header {
			display: flex;
			align-items: center;
			gap: 12px;
			padding: 12px 12px 12px 14px;
			min-height: 56px;

			&.conflicted {
				.status-badge {
					background-color: var(--clr-theme-warn-on-element);
					background-color: var(--clr-theme-warn-element);
				}
			}

			&.integrated {
				background-color: var(--clr-bg-1-muted);

				.status-badge {
					color: var(--clr-theme-purp-on-element);
					background-color: var(--clr-theme-purp-element);
				}
			}
		}

		.series-label {
			color: var(--clr-text-2);
		}

		.series-header-row {
			display: flex;
			align-items: center;
			justify-content: space-between;
			flex: 1;
		}

		/* NAME LABEL */
		.name-label-wrap {
			flex: 1;
			display: flex;
			align-items: center;
			gap: 10px;
			overflow: hidden;
		}

		.branch-status-info {
			flex: 1;
			display: flex;
			align-items: center;
			justify-content: space-between;
		}

		.select {
			max-width: 130px;
		}

		/* MODIFIERS */
		&.clear {
			background-color: var(--clr-bg-1);
		}
	}

	.series-branches {
		display: flex;
		flex-direction: column;
		margin-top: -10px;
	}

	.status-badge {
		padding: 3px 6px;
		height: 100%;
		border-radius: 100px;
		color: var(--clr-core-ntrl-100);
	}

	/* INTEGRATED LABEL */
	.integrated-label-wrap {
		display: flex;
		align-items: center;
		gap: 8px;
		padding-left: 6px;
		margin-right: 2px;
		color: var(--clr-text-2);
	}

	.integrated-label {
		white-space: nowrap;
	}

	.series-branch {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 14px;

		.branch-info {
			display: flex;
			align-items: center;
			gap: 8px;
			overflow: hidden;
			flex: 1;
		}
		&.conflicted {
			.status-badge {
				background-color: var(--clr-theme-warn-on-element);
				background-color: var(--clr-theme-warn-element);
			}
		}

		&.integrated {
			background-color: var(--clr-bg-1-muted);

			.status-badge {
				color: var(--clr-theme-purp-on-element);
				background-color: var(--clr-theme-purp-element);
			}
		}

		/* NESTING LINES */
		.structure-lines {
			position: relative;
			width: 20px;
			height: 20px;
			--line-color: var(--clr-border-2);
			--line-bounding-box: 12px;
			--line-horiz-offset: 0;

			&::before {
				content: '';
				position: absolute;
				top: -16px;
				right: var(--line-horiz-offset);
				width: var(--line-bounding-box);
				height: calc(100% + 8px);
				border-left: 1px solid var(--line-color);
				border-bottom: 1px solid var(--line-color);
			}

			&::after {
				content: '';
				position: absolute;
				top: 12px;
				right: var(--line-horiz-offset);
				width: var(--line-bounding-box);
				height: 20px;
				border-left: 1px solid var(--line-color);
			}

			&.last {
				&::after {
					display: none;
				}
			}
		}
	}
</style>

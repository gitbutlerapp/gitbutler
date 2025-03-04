<script lang="ts">
	import BranchLabel from '$components/BranchLabel.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import SeriesDescription from '$components/SeriesDescription.svelte';
	import SeriesHeaderStatusIcon from '$components/SeriesHeaderStatusIcon.svelte';
	import { getColorFromBranchType } from '$components/v3/lib';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { CommitStateType, StackBranch } from '$lib/branches/v3';
	import type { Snippet } from 'svelte';

	interface Props {
		projectId: string;
		stackId: string;
		branch: StackBranch;
		isTopBranch: boolean;
		readonly: boolean;
		lineColor?: string;
		children?: Snippet;
		onLabelDblClick?: () => void;
	}

	const {
		projectId,
		stackId,
		branch,
		isTopBranch,
		readonly,
		lineColor,
		children,
		onLabelDblClick
	}: Props = $props();

	const [stackService] = inject(StackService);

	const topCommitResult = $derived(
		stackService.commitAt(projectId, stackId, branch.name, 0).current
	);

	let seriesDescriptionEl = $state<HTMLTextAreaElement>();

	function editTitle(title: string) {
		console.log('FIXME', title);
	}

	function editDescription(description: string | null | undefined) {
		console.log('FIXME', description);
	}

	function toggleDescription() {
		console.log('FIXME');
	}
</script>

<div class="branch-header">
	{@render children?.()}
	<ReduxResult result={topCommitResult}>
		{#snippet children(commit)}
			{@const branchType: CommitStateType = commit?.state.type ?? 'LocalOnly'}
			{@const color = lineColor || getColorFromBranchType(branchType)}
			<div class="branch-info">
				<SeriesHeaderStatusIcon
					lineTop={isTopBranch ? false : true}
					icon={branchType === 'Integrated' ? 'tick-small' : 'branch-small'}
					iconColor="var(--clr-core-ntrl-100)"
					{color}
				/>
				<div class="branch-info__content">
					<div class="text-14 text-bold branch-info__name">
						{#if branch.remoteTrackingBranch}
							<span class="remote-name">
								{branch.remoteTrackingBranch}
							</span>
						{/if}
						<BranchLabel
							name={branch.name}
							onChange={(name) => editTitle(name)}
							readonly={readonly || !!branch.remoteTrackingBranch}
							onDblClick={() => {
								if (branchType !== 'Integrated') {
									onLabelDblClick?.();
								}
							}}
						/>
					</div>
					{#if branch.description}
						<div class="branch-info__description">
							<div class="branch-info__line" style:--bg-color={color}></div>
							<SeriesDescription
								bind:textAreaEl={seriesDescriptionEl}
								value={branch.description || ''}
								onBlur={(value) => editDescription(value)}
								onEmpty={() => toggleDescription()}
							/>
						</div>
					{/if}
				</div>
			</div>
		{/snippet}
	</ReduxResult>
</div>

<style lang="postcss">
	.branch-header {
		position: relative;
		display: flex;
		align-items: center;
		flex-direction: column;

		&:hover,
		&:focus-within {
			& :global(.branch-actions-menu) {
				--show: true;
			}
		}
	}

	.branch-info {
		width: 100%;
		padding-right: 14px;
		display: flex;
		justify-content: flex-start;
		align-items: center;

		.remote-name {
			min-width: max-content;
			padding: 0 0 0 2px;
			color: var(--clr-scale-ntrl-60);
		}
	}

	.branch-info__name {
		display: flex;
		align-items: center;
		justify-content: flex-start;
		min-width: 0;
		flex-grow: 1;
	}

	.branch-info__content {
		overflow: hidden;
		flex: 1;
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 14px 0;
		margin-left: -2px;
		text-overflow: ellipsis;
	}

	.branch-info__line {
		min-width: 2px;
		margin: 0 22px;
		background-color: var(--bg-color, var(--clr-border-3));
	}
</style>

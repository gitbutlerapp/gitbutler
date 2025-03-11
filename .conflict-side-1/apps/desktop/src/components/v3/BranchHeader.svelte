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
		selected: boolean;
		isTopBranch: boolean;
		readonly: boolean;
		lineColor?: string;
		children?: Snippet;
		actions?: Snippet;
		onclick: () => void;
		onLabelDblClick?: () => void;
	}

	const {
		projectId,
		stackId,
		branch,
		isTopBranch,
		readonly,
		lineColor,
		selected,
		children,
		actions,
		onclick,
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

<div class="branch-header" class:selected>
	{@render children?.()}
	<ReduxResult result={topCommitResult}>
		{#snippet children(commit)}
			{@const branchType: CommitStateType = commit?.state.type ?? 'LocalOnly'}
			{@const color = lineColor || getColorFromBranchType(branchType)}
			<div class="first-row" {onclick} role="button" onkeypress={onclick} tabindex="0">
				<SeriesHeaderStatusIcon
					lineTop={isTopBranch ? false : true}
					icon={branchType === 'Integrated' ? 'tick-small' : 'branch-small'}
					iconColor="var(--clr-core-ntrl-100)"
					{color}
				/>
				<div class="right">
					<div class="combined-name text-14 text-bold">
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
						<div class="description">
							<div class="line" style:--bg-color={color}></div>
							<SeriesDescription
								bind:textAreaEl={seriesDescriptionEl}
								value={branch.description || ''}
								onBlur={(value) => editDescription(value)}
								onEmpty={() => toggleDescription()}
							/>
						</div>
					{/if}
					{#if actions}
						{@render actions()}
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
		color: var(--clr-text-3);

		&:hover,
		&:focus-within {
			& :global(.branch-actions-menu) {
				--show: true;
			}
		}
	}

	.selected {
		color: var(--clr-text-2);
	}

	.first-row {
		width: 100%;
		padding-right: 14px;
		display: flex;
		justify-content: flex-start;
		align-items: center;
	}

	.combined-name {
		display: flex;
		align-items: center;
		justify-content: flex-start;
		min-width: 0;
		flex-grow: 1;
	}

	.right {
		overflow: hidden;
		flex: 1;
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 14px 0;
		margin-left: -2px;
		text-overflow: ellipsis;
	}

	.line {
		min-width: 2px;
		margin: 0 22px;
		background-color: var(--bg-color, var(--clr-border-3));
	}
</style>

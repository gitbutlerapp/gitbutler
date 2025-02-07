<script lang="ts">
	import BranchLabel from '$components/BranchLabel.svelte';
	import SeriesDescription from '$components/SeriesDescription.svelte';
	import SeriesHeaderStatusIcon from '$components/SeriesHeaderStatusIcon.svelte';
	import { getColorFromBranchType, isStackedBranch } from '$components/v3/lib';
	import type { CommitStateType, WorkspaceBranch } from '$lib/branches/v3';

	interface Props {
		branch: WorkspaceBranch;
		isTopBranch: boolean;
	}

	const { branch, isTopBranch }: Props = $props();

	const topPatch = $derived(
		isStackedBranch(branch.state) && branch?.state.subject.localAndRemote.length > 0
			? branch?.state.subject.localAndRemote[0]
			: undefined
	);

	let branchType = $derived(topPatch?.state.type ?? 'LocalOnly') as CommitStateType;
	const lineColor = $derived(getColorFromBranchType(branchType));

	const descriptionVisible = $derived(!!branch.description);
	const remoteName = $derived(
		branch.remoteTrackingBranch
			? branch.remoteTrackingBranch.replace('refs/remotes/', '').replace(`/${branch.name}`, '')
			: ''
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
	<div class="branch-info">
		<SeriesHeaderStatusIcon
			lineTop={isTopBranch ? false : true}
			icon={branchType === 'Integrated' ? 'tick-small' : 'branch-small'}
			iconColor="var(--clr-core-ntrl-100)"
			color={lineColor}
		/>
		<div class="branch-info__content">
			<div class="text-14 text-bold branch-info__name">
				{#if branch.remoteTrackingBranch}
					<span class="remote-name">
						{remoteName ? `${remoteName} /` : 'origin /'}
					</span>
				{/if}
				<BranchLabel
					name={branch.name}
					onChange={(name) => editTitle(name)}
					readonly={!!branch.remoteTrackingBranch}
					onDblClick={() => {
						if (branchType !== 'Integrated') {
							// stackingContextMenu?.showSeriesRenameModal?.(branch.name);
						}
					}}
				/>
			</div>
			{#if descriptionVisible}
				<div class="branch-info__description">
					<div class="branch-info__line" style:--bg-color={lineColor}></div>
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
</div>

<style lang="postcss">
	.branch-header {
		position: relative;
		display: flex;
		align-items: center;
		flex-direction: column;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-2);
		}

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
	}

	.branch-info__line {
		min-width: 2px;
		margin: 0 22px;
		background-color: var(--bg-color, var(--clr-border-3));
	}
</style>

<script lang="ts">
	import BranchLabel from '$components/BranchLabel.svelte';
	import SeriesDescription from '$components/SeriesDescription.svelte';
	import SeriesHeaderStatusIcon from '$components/SeriesHeaderStatusIcon.svelte';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import type { WorkspaceBranch } from '$lib/branches/v3';
	import type { CellType } from '@gitbutler/ui/commitLines/types';

	interface Props {
		branch: WorkspaceBranch;
		isTopBranch: boolean;
		// lastPush: Date | undefined;
	}

	const { branch, isTopBranch }: Props = $props();

	const topPatch = $derived(
		branch.state.type !== 'Archived' ? branch?.state.subject.localAndRemote[0] : undefined
	);

	// Lowercased first letter to match CellTypes from v2
	// TODO: Harmonize types once we drop v2
	let branchType = $derived(
		topPatch?.state?.type
			? topPatch?.state?.type[0]?.toLowerCase() + topPatch?.state?.type.slice(1)
			: 'local'
	) as CellType;
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
			icon={branchType === 'integrated' ? 'tick-small' : 'branch-small'}
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
						if (branchType !== 'integrated') {
							// stackingContextMenu?.showSeriesRenameModal?.(branch.name);
						}
					}}
				/>
			</div>
			{#if descriptionVisible}
				<div class="branch-info__description">
					<div class="branch-action__line" style:--bg-color={lineColor}></div>
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

	.branch-action {
		width: 100%;
		display: flex;
		justify-content: flex-start;
		align-items: stretch;

		.branch-action__body {
			width: 100%;
			padding: 0 14px 14px 0;
			display: flex;
			flex-direction: column;
			gap: 14px;
		}
	}

	.branch-action__line {
		min-width: 2px;
		margin: 0 22px 0 20px;
		background-color: var(--bg-color, var(--clr-border-3));
	}
</style>

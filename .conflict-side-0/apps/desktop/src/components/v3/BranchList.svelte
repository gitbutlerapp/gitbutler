<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import SeriesHeaderContextMenu from '$components/SeriesHeaderContextMenu.svelte';
	import BranchCard from '$components/v3/BranchCard.svelte';
	import BranchCommitList from '$components/v3/BranchCommitList.svelte';
	import CommitGoesHere from '$components/v3/CommitGoesHere.svelte';
	import CommitRow from '$components/v3/CommitRow.svelte';
	import NewBranchModal from '$components/v3/NewBranchModal.svelte';
	import PushButton from '$components/v3/PushButton.svelte';
	import { isLocalAndRemoteCommit, isUpstreamCommit } from '$components/v3/lib';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import {
		AmendCommitWithChangeDzHandler,
		SquashCommitDzHandler,
		type DzCommitData
	} from '$lib/commits/dropHandler';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId: string;
	};

	const { projectId, stackId }: Props = $props();
	const [stackService, uiState, baseBranchService, forge] = inject(
		StackService,
		UiState,
		BaseBranchService,
		DefaultForgeFactory
	);
	const branchesResult = $derived(stackService.branches(projectId, stackId));

	const baseBranchResponse = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseBranchResponse.current.data);
	const baseSha = $derived(base?.baseSha);

	const drawer = $derived(uiState.project(projectId).drawerPage);
	const isCommitting = $derived(drawer.current === 'new-commit');

	const selection = $derived(uiState.stack(stackId).selection.get());
	const selectedCommitId = $derived(selection.current?.commitId);

	let newBranchModal = $state<ReturnType<typeof NewBranchModal>>();
</script>

<ReduxResult {stackId} {projectId} result={branchesResult.current}>
	{#snippet children(branches, { stackId, projectId })}
		{#each branches as branch, i}
			{@const first = i === 0}
			{@const last = i === branches.length - 1}
			{@const branchName = branch.name}
			{@const localAndRemoteCommits = stackService.commits(projectId, stackId, branchName)}
			{@const upstreamOnlyCommits = stackService.upstreamCommits(projectId, stackId, branchName)}
			{@const isNewBranch =
				upstreamOnlyCommits.current.data?.length === 0 &&
				localAndRemoteCommits.current.data?.length === 0}
			<BranchCard {projectId} {stackId} branchName={branch.name} {first} {last} {isNewBranch}>
				{#snippet commitList()}
					<BranchCommitList {projectId} {stackId} {branchName} {selectedCommitId}>
						{#snippet upstreamTemplate({ commit, first, lastCommit, selected })}
							{@const commitId = commit.id}
							{#if !isCommitting}
								<CommitRow
									{stackId}
									{branchName}
									{projectId}
									{first}
									lastCommit={lastCommit && !commit}
									{commit}
									{selected}
									onclick={() => {
										uiState.stack(stackId).selection.set({ branchName, commitId, upstream: true });
										uiState.project(projectId).drawerPage.set(undefined);
									}}
								/>
							{/if}
						{/snippet}
						{#snippet localAndRemoteTemplate({ commit, first, last, lastCommit, selected })}
							{@const commitId = commit.id}
							{#if isCommitting}
								<!-- Only commits to the base can be `last`, see next `CommitGoesHere`. -->
								<CommitGoesHere
									{selected}
									{first}
									last={false}
									onclick={() => uiState.stack(stackId).selection.set({ branchName, commitId })}
								/>
							{/if}
							{@const dzCommit: DzCommitData = {
								id: commit.id,
								isRemote: isUpstreamCommit(commit),
								isIntegrated: isLocalAndRemoteCommit(commit) && commit.state.type === 'Integrated',
								isConflicted: isLocalAndRemoteCommit(commit) && commit.hasConflicts,
							}}
							{@const amendHandler = new AmendCommitWithChangeDzHandler(
								projectId,
								stackService,
								stackId,
								dzCommit,
								(newId) => uiState.stack(stackId).selection.set({ branchName, commitId: newId })
							)}
							{@const squashHandler = new SquashCommitDzHandler({
								stackService,
								projectId,
								stackId,
								commit: dzCommit
							})}
							<Dropzone handlers={[amendHandler, squashHandler]}>
								{#snippet overlay({ hovered, activated, handler })}
									{@const label =
										handler instanceof AmendCommitWithChangeDzHandler ? 'Amend' : 'Squash'}
									<CardOverlay {hovered} {activated} {label} />
								{/snippet}
								<CommitRow
									{stackId}
									{branchName}
									{projectId}
									{first}
									{lastCommit}
									lastBranch={last}
									{commit}
									{selected}
									draggable
									onclick={() => {
										const stackState = uiState.stack(stackId);
										stackState.selection.set({ branchName, commitId });
										stackState.activeSelectionId.set({ type: 'commit', commitId });
										uiState.project(projectId).drawerPage.set(undefined);
									}}
								/>
							</Dropzone>
							{#if isCommitting && last}
								<CommitGoesHere
									{first}
									{last}
									selected={selectedCommitId === baseSha}
									onclick={() =>
										uiState.stack(stackId).selection.set({ branchName, commitId: baseSha })}
								/>
							{/if}
						{/snippet}
					</BranchCommitList>
				{/snippet}
				{#snippet contextMenu({
					branchType,
					branchName,
					trackingBranch,
					leftClickTrigger,
					rightClickTrigger,
					onToggle,
					addListener
				})}
					{@const forgeBranch = trackingBranch ? forge.current?.branch(trackingBranch) : undefined}
					<SeriesHeaderContextMenu
						{projectId}
						{stackId}
						{leftClickTrigger}
						{rightClickTrigger}
						{onToggle}
						{branchType}
						{addListener}
						branchName={branch.name}
						seriesCount={branches.length}
						isTopBranch={first}
						descriptionOption={false}
						onGenerateBranchName={() => {
							throw new Error('Not implemented!');
						}}
						onAddDependentSeries={() => newBranchModal?.show(branchName)}
						onOpenInBrowser={() => {
							const url = forgeBranch?.url;
							if (url) openExternalUrl(url);
						}}
						isPushed={!!branch.remoteTrackingBranch}
					/>
				{/snippet}
			</BranchCard>
		{/each}
		<PushButton {projectId} {stackId} multipleBranches={branches.length > 0} />
	{/snippet}
</ReduxResult>

<NewBranchModal {projectId} {stackId} bind:this={newBranchModal} />

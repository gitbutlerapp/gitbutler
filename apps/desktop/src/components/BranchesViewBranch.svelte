<script lang="ts">
	import BranchCard from '$components/BranchCard.svelte';
	import BranchesViewCommitContextMenu from '$components/BranchesViewCommitContextMenu.svelte';
	import CherryApplyModal from '$components/CherryApplyModal.svelte';
	import CommitRow from '$components/CommitRow.svelte';
	import NestedChangedFiles from '$components/NestedChangedFiles.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { commitCreatedAt, type Commit } from '$lib/branches/v3';
	import { createCommitSelection } from '$lib/selection/key';
	import { getColorFromPushStatus, pushStatusToIcon, type BranchDetails } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';

	type Props = {
		projectId: string;
		stackId?: string;
		branchName: string;
		remote?: string;
		isTopBranch?: boolean;
		isTarget?: boolean;
		inWorkspace?: boolean;
		selectedCommitId?: string;
		onCommitClick: (commitId: string) => void;
		onFileClick: (index: number) => void;
		onerror?: (error: unknown) => void;
	};

	const {
		projectId,
		stackId,
		branchName,
		remote,
		isTopBranch = true,
		inWorkspace,
		isTarget,
		selectedCommitId,
		onCommitClick,
		onFileClick,
		onerror
	}: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const branchQuery = $derived(
		stackId
			? stackService.branchDetails(projectId, stackId, branchName)
			: stackService.unstackedBranchDetails(projectId, branchName, remote)
	);

	let cherryApplyModal = $state<CherryApplyModal>();
</script>

<ReduxResult result={branchQuery.result} {projectId} {stackId} {onerror}>
	{#snippet children(branch, env)}
		{@render branchCard(branch, env)}
	{/snippet}
</ReduxResult>

{#snippet commitMenu(rightClickTrigger: HTMLElement)}
	<BranchesViewCommitContextMenu
		{rightClickTrigger}
		onCherryPick={() => {
			cherryApplyModal?.open();
		}}
	/>
{/snippet}

{#snippet renderCommitRow(commit: Commit, idx: number, totalLocal: number)}
	{#snippet menu({ rightClickTrigger }: { rightClickTrigger: HTMLElement })}
		{@render commitMenu(rightClickTrigger)}
	{/snippet}
	{@const commitType: 'LocalOnly' | 'LocalAndRemote' | 'Integrated' = commit.state.type}
	{@const isDiverged = commit.state.type === 'LocalAndRemote' && commit.id !== commit.state.subject}
	{@const shouldShowMenu = !(inWorkspace || isTarget)}
	{@const isLastCommit = idx === totalLocal - 1}
	<CommitRow
		disableCommitActions={false}
		{stackId}
		type={commitType}
		diverged={isDiverged}
		commitMessage={commit.message}
		gerritReviewUrl={commit?.gerritReviewUrl ?? undefined}
		createdAt={commitCreatedAt(commit)}
		commitId={commit.id}
		{branchName}
		selected={commit.id === selectedCommitId}
		active={commit.id === selectedCommitId}
		lastCommit={isLastCommit}
		onclick={() => {
			onCommitClick(commit.id);
		}}
		{...shouldShowMenu && { menu }}
	>
		{#snippet changedFiles()}
			{@const changesQuery = stackService.commitChanges(projectId, commit.id)}

			<ReduxResult {projectId} {stackId} result={changesQuery.result}>
				{#snippet children(changesResult)}
					<NestedChangedFiles
						title="Changed files"
						{projectId}
						{stackId}
						draggableFiles
						selectionId={createCommitSelection({ commitId: commit.id, stackId })}
						changes={changesResult.changes.filter(
							(change) => !(change.path in (changesResult.conflictEntries?.entries ?? {}))
						)}
						stats={changesResult.stats}
						conflictEntries={changesResult.conflictEntries}
						autoselect
						allowUnselect={false}
						{onFileClick}
					/>
				{/snippet}
			</ReduxResult>
		{/snippet}
	</CommitRow>
{/snippet}

{#snippet branchCard(branch: BranchDetails, env: { projectId: string; stackId?: string })}
	{@const commitColor = getColorFromPushStatus(branch.pushStatus)}
	{@const localCount = branch.commits?.length ?? 0}
	{@const hasCommits = localCount > 0}

	<BranchCard
		type="normal-branch"
		first={isTopBranch}
		lineColor={commitColor}
		projectId={env.projectId}
		branchName={branch.name}
		{isTopBranch}
		isNewBranch={localCount === 0}
		iconName={pushStatusToIcon(branch.pushStatus)}
		trackingBranch={branch.remoteTrackingBranch || undefined}
		readonly
		roundedBottom={!hasCommits}
		selected={false}
		disableClick
	>
		{#snippet branchContent()}
			{#if hasCommits}
				<div class="branch-commits">
					{#each branch.commits ?? [] as commit, idx}
						{@render renderCommitRow(commit, idx, localCount)}
					{/each}
				</div>
			{/if}
		{/snippet}
	</BranchCard>
{/snippet}

<CherryApplyModal bind:this={cherryApplyModal} {projectId} subject={selectedCommitId} />

<style lang="postcss">
	.branch-commits {
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
	}
</style>

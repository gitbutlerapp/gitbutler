<script lang="ts">
	import BranchCard from "$components/branch/BranchCard.svelte";
	import BranchesViewCommitContextMenu from "$components/branchesPage/BranchesViewCommitContextMenu.svelte";
	import CherryApplyModal from "$components/commit/CherryApplyModal.svelte";
	import CommitListItem from "$components/commit/CommitListItem.svelte";
	import ChangedFilesPanel from "$components/files/ChangedFilesPanel.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { commitCreatedAt } from "$lib/branches/v3";
	import { createCommitSelection } from "$lib/selection/key";
	import { getColorFromPushStatus, pushStatusToIcon } from "$lib/stacks/stack";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import type { BranchDetails, Commit, Segment } from "@gitbutler/but-sdk";

	type Props = {
		projectId: string;
		stackId?: string;
		branchName?: string;
		segment?: Segment;
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
		segment,
		remote,
		isTopBranch = true,
		inWorkspace,
		isTarget,
		selectedCommitId,
		onCommitClick,
		onFileClick,
		onerror,
	}: Props = $props();

	const stackService = inject(STACK_SERVICE);
	let cherryApplyModal = $state<CherryApplyModal>();
</script>

{#if segment}
	{@render branchCard(segment, { projectId, stackId })}
{:else if stackId && branchName}
	{@const branchQuery = stackService.branchDetails(projectId, stackId, branchName)}
	<ReduxResult result={branchQuery.result} {projectId} {stackId} {onerror}>
		{#snippet children(branch, env)}
			{@render branchCard(branch, env)}
		{/snippet}
	</ReduxResult>
{:else if branchName}
	{@const branchQuery = stackService.unstackedBranchDetails(projectId, branchName, remote)}
	<ReduxResult result={branchQuery.result} {projectId} {stackId} {onerror}>
		{#snippet children(branch, env)}
			{@render branchCard(branch, env)}
		{/snippet}
	</ReduxResult>
{/if}

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
	{@const isDiverged = commit.state.type === "LocalAndRemote" && commit.id !== commit.state.subject}
	{@const shouldShowMenu = !(inWorkspace || isTarget)}
	{@const isLastCommit = idx === totalLocal - 1}
	<CommitListItem
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
					<ChangedFilesPanel
						title="Changed files"
						{projectId}
						{stackId}
						draggableFiles
						selectionId={createCommitSelection({ commitId: commit.id, stackId })}
						changes={changesResult.changes.filter(
							(change) => !(change.path in (changesResult.conflictEntries?.entries ?? {})),
						)}
						stats={changesResult.stats ?? undefined}
						conflictEntries={changesResult.conflictEntries}
						autoselect
						allowUnselect={false}
						{onFileClick}
					/>
				{/snippet}
			</ReduxResult>
		{/snippet}
	</CommitListItem>
{/snippet}

{#snippet branchCard(branch: BranchDetails | Segment, env: { projectId: string; stackId?: string })}
	{@const commitColor = getColorFromPushStatus(branch.pushStatus)}
	{@const localCount = branch.commits?.length ?? 0}
	{@const hasCommits = localCount > 0}
	{@const trackingBranch =
		"refName" in branch
			? branch.remoteTrackingRefName
				? new TextDecoder().decode(new Uint8Array(branch.remoteTrackingRefName.fullNameBytes))
				: undefined
			: branch.remoteTrackingBranch || undefined}
	{@const displayBranchName =
		branchName ?? ("refName" in branch ? branch.refName?.displayName : branch.name)}

	<BranchCard
		type="normal-branch"
		first={isTopBranch}
		lineColor={commitColor}
		projectId={env.projectId}
		branchName={displayBranchName ?? "Unnamed segment"}
		{isTopBranch}
		isNewBranch={localCount === 0}
		iconName={pushStatusToIcon(branch.pushStatus)}
		{trackingBranch}
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
		border: 1px solid var(--border-2);
		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
	}
</style>

<script lang="ts">
	import BranchCard from '$components/BranchCard.svelte';
	import BranchesViewCommitContextMenu from '$components/BranchesViewCommitContextMenu.svelte';
	import CherryApplyModal from '$components/CherryApplyModal.svelte';
	import CommitRow from '$components/CommitRow.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { BranchesSelectionActions } from '$lib/branches/branchesSelection';
	import { commitCreatedAt, type Commit } from '$lib/branches/v3';
	import { getColorFromPushStatus, pushStatusToIcon, type BranchDetails } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';

	type Props = {
		projectId: string;
		stackId?: string;
		branchName: string;
		remote?: string;
		isTopBranch?: boolean;
		inWorkspace: boolean;
		hasLocal: boolean;
		onerror?: (error: unknown) => void;
	};

	const {
		projectId,
		stackId,
		branchName,
		remote,
		isTopBranch = true,
		inWorkspace,
		hasLocal,
		onerror
	}: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const branchQuery = $derived(
		stackId
			? stackService.branchDetails(projectId, stackId, branchName)
			: stackService.unstackedBranchDetails(projectId, branchName, remote)
	);

	const uiState = inject(UI_STATE);
	const projectState = $derived(uiState.project(projectId));
	const branchesSelection = $derived(projectState.branchesSelection);

	let cherryApplyModal = $state<CherryApplyModal>();
	let selectedCommitId = $state<string>();
</script>

<ReduxResult result={branchQuery.result} {projectId} {stackId} {onerror}>
	{#snippet children(branch, env)}
		{@render branchCard(branch, env)}
	{/snippet}
</ReduxResult>

{#snippet commitMenu(rightClickTrigger: HTMLElement, commitId: string)}
	<BranchesViewCommitContextMenu
		{rightClickTrigger}
		onCherryPick={() => {
			selectedCommitId = commitId;
			cherryApplyModal?.open();
		}}
	/>
{/snippet}

{#snippet renderCommitRow(
	commit: Commit,
	idx: number,
	totalLocal: number,
	branchFirstCommitType: string | undefined
)}
	{#snippet menu({ rightClickTrigger }: { rightClickTrigger: HTMLElement })}
		{@render commitMenu(rightClickTrigger, commit.id)}
	{/snippet}
	{@const localCommit = commit}
	{@const commitType: 'LocalOnly' | 'LocalAndRemote' | 'Integrated' = (branchFirstCommitType as 'LocalOnly' | 'LocalAndRemote' | 'Integrated') ?? 'LocalOnly'}
	{@const isDiverged =
		localCommit.state.type === 'LocalAndRemote' && commit.id !== localCommit.state.subject}
	{@const shouldShowMenu = !(
		branchesSelection.current.inWorkspace || branchesSelection.current.isTarget
	)}
	{@const isLastCommit = idx === totalLocal - 1}
	<CommitRow
		disableCommitActions={false}
		{stackId}
		type={commitType}
		diverged={isDiverged}
		commitMessage={commit.message}
		gerritReviewUrl={localCommit?.gerritReviewUrl ?? undefined}
		createdAt={commitCreatedAt(commit)}
		commitId={commit.id}
		{branchName}
		selected={commit.id === branchesSelection?.current.commitId}
		onclick={() => {
			BranchesSelectionActions.selectCommit(branchesSelection, {
				commitId: commit.id,
				remote
			});
		}}
		lastCommit={isLastCommit}
		{...shouldShowMenu && { menu }}
	/>
{/snippet}

{#snippet branchCard(branch: BranchDetails, env: { projectId: string; stackId?: string })}
	{@const commitColor = getColorFromPushStatus(branch.pushStatus)}
	{@const localCount = branch.commits?.length ?? 0}
	{@const hasCommits = localCount > 0}
	{@const isBranchSelected =
		branchesSelection.current.branchName === branch.name &&
		branchesSelection.current.stackId === env.stackId &&
		!branchesSelection.current.commitId}
	{@const branchFirstCommitType = branch.commits?.at(0)?.state.type}

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
		selected={isBranchSelected}
		onclick={() => {
			if (env.stackId) {
				BranchesSelectionActions.selectStack(branchesSelection, {
					stackId: env.stackId,
					branchName,
					inWorkspace,
					hasLocal,
					prNumber: branch.prNumber ?? undefined
				});
			} else {
				BranchesSelectionActions.selectBranch(branchesSelection, {
					branchName,
					remote,
					hasLocal,
					prNumber: branch.prNumber ?? undefined
				});
			}
		}}
	>
		{#snippet branchContent()}
			{#if hasCommits}
				<div class="branch-commits">
					{#each branch.commits ?? [] as commit, idx}
						{@render renderCommitRow(commit, idx, localCount, branchFirstCommitType)}
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

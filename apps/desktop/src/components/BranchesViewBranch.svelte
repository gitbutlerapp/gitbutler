<script lang="ts">
	import BranchCard from '$components/BranchCard.svelte';
	import BranchesViewCommitContextMenu from '$components/BranchesViewCommitContextMenu.svelte';
	import CherryApplyModal from '$components/CherryApplyModal.svelte';
	import CommitRow from '$components/CommitRow.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { BranchesSelectionActions } from '$lib/branches/branchesSelection';
	import { commitCreatedAt } from '$lib/branches/v3';
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
		{#if stackId}
			{@render branchCard(branch, env)}
		{:else}
			{@render branchCard(branch, env)}
		{/if}
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

{#snippet branchCard(branch: BranchDetails, env: { projectId: string; stackId?: string })}
	{@const commitColor = getColorFromPushStatus(branch.pushStatus)}
	<BranchCard
		type="normal-branch"
		first={isTopBranch}
		lineColor={commitColor}
		projectId={env.projectId}
		branchName={branch.name}
		{isTopBranch}
		isNewBranch={branch.commits?.length === 0}
		iconName={pushStatusToIcon(branch.pushStatus)}
		trackingBranch={branch.remoteTrackingBranch || undefined}
		readonly
		selected={branchesSelection.current.branchName === branch.name &&
			branchesSelection.current.stackId === env.stackId &&
			!branchesSelection.current.commitId}
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
			<div class="branch-commits hide-when-empty">
				{#each branch.upstreamCommits || [] as commit, idx}
					{#snippet menu({ rightClickTrigger }: { rightClickTrigger: HTMLElement })}
						{@render commitMenu(rightClickTrigger, commit.id)}
					{/snippet}
					<CommitRow
						disableCommitActions={false}
						stackId={env.stackId}
						type="Remote"
						active
						commitMessage={commit.message}
						createdAt={commitCreatedAt(commit)}
						commitId={commit.id}
						branchName={branch.name}
						selected={commit.id === branchesSelection?.current.commitId}
						onclick={() => {
							BranchesSelectionActions.selectCommit(branchesSelection, {
								commitId: commit.id,
								remote
							});
						}}
						lastCommit={idx === branch.upstreamCommits.length - 1 && branch.commits.length === 0}
						menu={branchesSelection.current.inWorkspace || branchesSelection.current.isTarget
							? undefined
							: menu}
					></CommitRow>
				{/each}
				{#each branch.commits || [] as commit, idx}
					{#snippet menu({ rightClickTrigger }: { rightClickTrigger: HTMLElement })}
						{@render commitMenu(rightClickTrigger, commit.id)}
					{/snippet}
					<CommitRow
						disableCommitActions={false}
						stackId={env.stackId}
						type={branch.commits.at(0)?.state.type || 'LocalOnly'}
						diverged={commit.state.type === 'LocalAndRemote' && commit.id !== commit.state.subject}
						commitMessage={commit.message}
						gerritReviewUrl={commit.gerritReviewUrl ?? undefined}
						createdAt={commitCreatedAt(commit)}
						commitId={commit.id}
						branchName={branch.name}
						selected={commit.id === branchesSelection?.current.commitId}
						onclick={() => {
							BranchesSelectionActions.selectCommit(branchesSelection, {
								commitId: commit.id,
								remote
							});
						}}
						lastCommit={idx === branch.commits.length - 1}
						active
						menu={branchesSelection.current.inWorkspace || branchesSelection.current.isTarget
							? undefined
							: menu}
					></CommitRow>
				{/each}
			</div>
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

<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchCard from '$components/v3/BranchCard.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import CommitRow from '$components/v3/CommitRow.svelte';
	import { pushStatusToColor, pushStatusToIcon } from '$lib/stacks/stack';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';

	type Props = {
		projectId: string;
		stackId?: string;
		branchName: string;
		isTopBranch?: boolean;
	};

	const { projectId, stackId, branchName, isTopBranch = true }: Props = $props();

	const stackService = getContext(StackService);
	const branchResult = $derived(
		stackId
			? stackService.branchDetails(projectId, stackId, branchName)
			: stackService.unstackedBranchDetails(projectId, branchName)
	);

	const uiState = getContext(UiState);
	const projectState = $derived(uiState.project(projectId));
	const branchesState = $derived(projectState.branchesSelection);
</script>

<ReduxResult result={branchResult.current} {projectId} {stackId}>
	{#snippet children(branch, env)}
		<BranchCard type="normal-branch" projectId={env.projectId} branchName={branch.name}>
			{#snippet header()}
				<BranchHeader
					type="normal-branch"
					{branchName}
					{projectId}
					{isTopBranch}
					iconName={pushStatusToIcon(branch.pushStatus)}
					lineColor={getColorFromBranchType(pushStatusToColor(branch.pushStatus))}
					trackingBranch={branch.remoteTrackingBranch || undefined}
					isCommitting={false}
					readonly
					selected={branchesState.current.branchName === branch.name &&
						branchesState.current.stackId === env.stackId &&
						!branchesState.current.commitId}
					onclick={() => {
						branchesState.current = {
							branchName,
							stackId: env.stackId
						};
					}}
				/>
			{/snippet}
			{#snippet commitList()}
				{#each branch.upstreamCommits || [] as commit, idx}
					<CommitRow
						disableCommitActions
						type="Remote"
						commitMessage={commit.message}
						createdAt={commit.createdAt}
						commitId={commit.id}
						projectId={env.projectId}
						branchName={branch.name}
						selected={commit.id === branchesState?.current.commitId}
						onclick={() => {
							branchesState.set({
								stackId: env.stackId,
								branchName: branch.name,
								commitId: commit.id
							});
						}}
						lastCommit={idx === branch.upstreamCommits.length - 1 && branch.commits.length === 0}
					/>
				{/each}
				{#each branch.commits || [] as commit, idx}
					<CommitRow
						disableCommitActions
						type="LocalAndRemote"
						commitMessage={commit.message}
						createdAt={commit.createdAt}
						commitId={commit.id}
						projectId={env.projectId}
						branchName={branch.name}
						selected={commit.id === branchesState?.current.commitId}
						onclick={() => {
							branchesState.set({
								stackId: env.stackId,
								branchName: branch.name,
								commitId: commit.id
							});
						}}
						lastCommit={idx === branch.commits.length - 1}
					/>
				{/each}
			{/snippet}
		</BranchCard>
	{/snippet}
</ReduxResult>

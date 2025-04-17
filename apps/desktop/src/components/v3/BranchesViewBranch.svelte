<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchCard from '$components/v3/BranchCard.svelte';
	import CommitRow from '$components/v3/CommitRow.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId?: string;
		branchName: string;
	};

	const { projectId, stackId, branchName }: Props = $props();

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
		<BranchCard
			type="normal-branch"
			projectId={env.projectId}
			iconName="branch-local"
			branchName={branch.name}
			stackId={env.stackId}
		>
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

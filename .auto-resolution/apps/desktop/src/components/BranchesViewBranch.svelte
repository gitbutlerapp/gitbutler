<script lang="ts">
	import BranchCard from '$components/BranchCard.svelte';
	import CommitRow from '$components/CommitRow.svelte';
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { pushStatusToColor, pushStatusToIcon, type BranchDetails } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';

	type Props = {
		projectId: string;
		stackId?: string;
		branchName: string;
		remote?: string;
		isTopBranch?: boolean;
		onerror?: (error: unknown) => void;
	};

	const { projectId, stackId, branchName, remote, isTopBranch = true, onerror }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const branchResult = $derived(
		stackId
			? stackService.branchDetails(projectId, stackId, branchName)
			: stackService.unstackedBranchDetails(projectId, branchName, remote)
	);

	const uiState = inject(UI_STATE);
	const projectState = $derived(uiState.project(projectId));
	const branchesState = $derived(projectState.branchesSelection);
</script>

<ReduxResult result={branchResult.current} {projectId} {stackId} {onerror}>
	{#snippet children(branch, env)}
		{#if stackId}
			{@render branchCard(branch, env)}
		{:else}
			<ConfigurableScrollableContainer>
				{@render branchCard(branch, env)}
			</ConfigurableScrollableContainer>
		{/if}
	{/snippet}
</ReduxResult>

{#snippet branchCard(branch: BranchDetails, env: { projectId: string; stackId?: string })}
	{@const commitColor = getColorFromBranchType(pushStatusToColor(branch.pushStatus))}
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
		selected={branchesState.current.branchName === branch.name &&
			branchesState.current.stackId === env.stackId &&
			!branchesState.current.commitId}
		onclick={() => {
			branchesState.set({
				branchName,
				stackId: env.stackId,
				remote
			});
		}}
	>
		{#snippet branchContent()}
			<div class="branch-commits hide-when-empty">
				{#each branch.upstreamCommits || [] as commit, idx}
					<CommitRow
						disableCommitActions
						type="Remote"
						active
						commitMessage={commit.message}
						createdAt={commit.createdAt}
						commitId={commit.id}
						branchName={branch.name}
						selected={commit.id === branchesState?.current.commitId}
						onclick={() => {
							branchesState.set({
								stackId: env.stackId,
								branchName: branchName,
								commitId: commit.id,
								remote
							});
						}}
						lastCommit={idx === branch.upstreamCommits.length - 1 && branch.commits.length === 0}
					/>
				{/each}
				{#each branch.commits || [] as commit, idx}
					<CommitRow
						disableCommitActions
						type={branch.commits.at(0)?.state.type || 'LocalOnly'}
						diverged={commit.state.type === 'LocalAndRemote' && commit.id !== commit.state.subject}
						commitMessage={commit.message}
						createdAt={commit.createdAt}
						commitId={commit.id}
						branchName={branch.name}
						selected={commit.id === branchesState?.current.commitId}
						onclick={() => {
							branchesState.set({
								stackId: env.stackId,
								branchName: branchName,
								commitId: commit.id,
								remote
							});
						}}
						lastCommit={idx === branch.commits.length - 1}
						active
					/>
				{/each}
			</div>
		{/snippet}
	</BranchCard>
{/snippet}

<style lang="postcss">
	.branch-commits {
		border-top: 1px solid var(--clr-border-2);
	}
</style>

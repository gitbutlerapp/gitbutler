<script lang="ts">
	import BranchCommitList from './BranchCommitList.svelte';
	import BranchHeader from './BranchHeader.svelte';
	import CommitRow from './CommitRow.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
	};

	const { projectId, stackId, branchName }: Props = $props();
	const [stackService] = inject(StackService, BaseBranchService);

	const branchesResult = $derived(stackService.branches(projectId, stackId));
</script>

<div class="commit-goes-here">
	<ReduxResult result={branchesResult.current}>
		{#snippet children(branches)}
			{#each branches as branch, i}
				<div class="branch" class:selected={branch.name === branchName}>
					<div class="header-wrapper">
						<BranchHeader
							{projectId}
							{stackId}
							{branch}
							isTopBranch={i === 0}
							lineColor="var(--clr-commit-local)"
							readonly
						/>
					</div>
					<BranchCommitList {projectId} {stackId} branchName={branch.name}>
						{#snippet localAndRemoteTemplate({ commit, commitKey, first, lastCommit })}
							<CommitRow
								{projectId}
								{commitKey}
								{first}
								{commit}
								{lastCommit}
								lineColor="var(--clr-commit-local)"
								opacity={0.4}
							/>
						{/snippet}
					</BranchCommitList>
				</div>
			{/each}
		{/snippet}
	</ReduxResult>
</div>

<style lang="postcss">
	.commit-goes-here {
		display: flex;
		flex-direction: column;
	}

	.branch {
		display: flex;
		flex-direction: column;
		margin-bottom: 14px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-2);
		&.selected {
			background-color: var(--clr-bg-1);
		}
	}
	.header-wrapper {
		opacity: 0.4;
	}
	.selected .header-wrapper {
		opacity: 1;
	}
</style>

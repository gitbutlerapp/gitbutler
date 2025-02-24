<script lang="ts">
	import BranchCommitList from './BranchCommitList.svelte';
	import BranchHeader from './BranchHeader.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { createCommitPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
		parentId: string;
	};

	const { projectId, stackId, branchName, parentId }: Props = $props();
	const [stackService] = inject(StackService);

	const branchesResult = $derived(stackService.branches(projectId, stackId));
</script>

<div class="commit-goes-here">
	<ReduxResult result={branchesResult.current}>
		{#snippet children(branches)}
			{#each branches as branch, i}
				<div class="branch">
					<BranchHeader {projectId} {stackId} {branch} isTopBranch={i === 0} readonly />
					<BranchCommitList
						{projectId}
						{stackId}
						branchName={branch.name}
						selectedCommitId={parentId}
						onclick={(commitId) =>
							goto(createCommitPath(projectId, stackId, branchName, commitId), {
								replaceState: true
							})}
					/>
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
		margin: 14px 14px 0 14px;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
	}
</style>

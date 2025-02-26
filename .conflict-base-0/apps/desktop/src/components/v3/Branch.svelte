<script lang="ts">
	import BranchDividerLine from './BranchDividerLine.svelte';
	import CommitRow from './CommitRow.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchCommitList from '$components/v3/BranchCommitList.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import { branchPath, commitPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { inject } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
		first: boolean;
		last: boolean;
		selected: boolean;
		selectedCommitId?: string;
	}

	let {
		projectId,
		stackId,
		branchName,
		first,
		last,
		selected,
		selectedCommitId = $bindable()
	}: Props = $props();

	const [stackService] = inject(StackService);
	const branchResult = $derived(stackService.branchByName(projectId, stackId, branchName).current);
	const commitResult = $derived(stackService.commitAt(projectId, stackId, branchName, 0).current);
</script>

<ReduxResult result={combineResults(branchResult, commitResult)}>
	{#snippet children([branch, commit])}
		{#if !first}
			<BranchDividerLine topPatchStatus={commit?.state.type ?? 'LocalOnly'} />
		{/if}
		<div class="branch" class:selected data-series-name={branchName}>
			<BranchHeader
				{projectId}
				{stackId}
				{branch}
				isTopBranch={first}
				readonly={false}
				onclick={() => goto(branchPath(projectId, stackId, branch.name))}
			/>
			<BranchCommitList {projectId} {stackId} {branchName} lastBranch={last} {selectedCommitId}>
				{#snippet upstreamTemplate({ commit, commitKey, first, last, selected })}
					<CommitRow
						{projectId}
						{commitKey}
						{first}
						{last}
						{commit}
						{selected}
						onclick={() => goto(commitPath(projectId, commitKey))}
					/>
				{/snippet}
				{#snippet localAndRemoteTemplate({ commit, commitKey, first, last, selected })}
					<CommitRow
						{projectId}
						{commitKey}
						{first}
						{last}
						{commit}
						{selected}
						onclick={() => goto(commitPath(projectId, commitKey))}
					/>
				{/snippet}
			</BranchCommitList>
		</div>
	{/snippet}
</ReduxResult>

<style>
	.branch {
		display: flex;
		flex-direction: column;
		position: relative;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
	}
</style>

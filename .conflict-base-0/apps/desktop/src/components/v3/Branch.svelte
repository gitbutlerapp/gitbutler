<script lang="ts">
	import BranchDividerLine from './BranchDividerLine.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchCommitList from '$components/v3/BranchCommitList.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { inject } from '@gitbutler/shared/context';

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
			<BranchHeader {projectId} {stackId} {branch} isTopBranch={first} />
			<BranchCommitList {projectId} {stackId} {branchName} lastBranch={last} {selectedCommitId} />
		</div>
	{/snippet}
</ReduxResult>

<style>
	.branch {
		position: relative;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-2);
		&.selected {
			background: var(--clr-bg-1);
		}
	}
</style>

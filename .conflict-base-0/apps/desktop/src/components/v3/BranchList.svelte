<script lang="ts">
	import Branch from './Branch.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId: string;
		selectedBranchName: string;
		selectedCommitId?: string;
	};

	const { projectId, stackId, selectedBranchName, selectedCommitId }: Props = $props();
	const [stackService] = inject(StackService);

	const result = $derived(stackService.branches(projectId, stackId));
</script>

<ReduxResult result={result.current}>
	{#snippet children(branches)}
		{#each branches as branch, i (branch.name)}
			{@const first = i === 0}
			{@const last = i === branches.length - 1}
			<Branch
				{projectId}
				{stackId}
				branchName={branch.name}
				selected={selectedBranchName === branch.name}
				{selectedCommitId}
				{first}
				{last}
			/>
		{/each}
	{/snippet}
</ReduxResult>

<style lang="postcss">
</style>

<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchesViewBranch from '$components/v3/BranchesViewBranch.svelte';
	import { getStackBranchNames } from '$lib/stacks/stack';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId: string;
	};

	const { projectId, stackId }: Props = $props();

	const stackService = getContext(StackService);

	const stackResult = $derived(stackService.allStackById(projectId, stackId));
</script>

<ReduxResult result={stackResult.current} {projectId} {stackId}>
	{#snippet children(stack, { stackId, projectId })}
		<div class="flex flex-col gap-8">
			{#each getStackBranchNames(stack) as branchName, idx}
				<BranchesViewBranch {projectId} {stackId} {branchName} isTopBranch={idx === 0} />
			{/each}
		</div>
	{/snippet}
</ReduxResult>

<style>
	.flex {
		display: flex;
	}

	.flex-col {
		flex-direction: column;
	}

	.gap-8 {
		gap: 8px;
	}
</style>

<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchesViewBranch from '$components/v3/BranchesViewBranch.svelte';
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
	{#snippet children(stack, env)}
		<div class="flex flex-col gap-8">
			{#each stack.branchNames || [] as branchName, idx}
				<BranchesViewBranch
					projectId={env.projectId}
					stackId={env.stackId}
					{branchName}
					isTopBranch={idx === 0}
				/>
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

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
		<div class="branches-wrapper">
			{#each getStackBranchNames(stack) as branchName, idx}
				<BranchesViewBranch {projectId} {stackId} {branchName} isTopBranch={idx === 0} />
			{/each}
		</div>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.branches-wrapper {
		display: flex;
		flex-direction: column;
		flex: 1;
		overflow: hidden;
		padding: 12px;
		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		background-image: radial-gradient(
			oklch(from var(--clr-scale-ntrl-50) l c h / 0.5) 0.6px,
			#ffffff00 0.6px
		);
		background-size: 6px 6px;
	}
</style>

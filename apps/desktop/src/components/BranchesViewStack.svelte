<script lang="ts">
	import BranchDividerLine from '$components/BranchDividerLine.svelte';
	import BranchesViewBranch from '$components/BranchesViewBranch.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { getColorFromPushStatus, getStackBranchNames } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';

	type Props = {
		projectId: string;
		stackId: string;
		inWorkspace: boolean;
		hasLocal: boolean;
		onerror: (err: unknown) => void;
	};

	const { projectId, stackId, inWorkspace, hasLocal, onerror }: Props = $props();

	const stackService = inject(STACK_SERVICE);

	const stackQuery = $derived(stackService.allStackById(projectId, stackId));
</script>

<ReduxResult result={stackQuery.result} {projectId} {stackId} {onerror}>
	{#snippet children(stack, { stackId, projectId })}
		{#if stack === null}
			<p>Stack not found.</p>
		{:else}
			{#each getStackBranchNames(stack) as branchName, idx}
				{@const branchDetailsQuery = stackService.branchDetails(projectId, stackId, branchName)}
				{@const branchDetails = branchDetailsQuery.response}
				{@const lineColor = branchDetails
					? getColorFromPushStatus(branchDetails.pushStatus)
					: 'var(--clr-commit-local)'}

				{#if idx > 0}
					<BranchDividerLine {lineColor} />
				{/if}

				<BranchesViewBranch
					{projectId}
					{stackId}
					{branchName}
					isTopBranch={idx === 0}
					{inWorkspace}
					{hasLocal}
					{onerror}
				/>
			{/each}
		{/if}
	{/snippet}
</ReduxResult>

<script lang="ts">
	import BranchDividerLine from "$components/branch/BranchDividerLine.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import BranchesViewBranch from "$components/views/BranchesViewBranch.svelte";
	import { getColorFromPushStatus } from "$lib/stacks/stack";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";

	type Props = {
		projectId: string;
		stackId: string;
		inWorkspace: boolean;
		isTarget?: boolean;
		selectedCommitId?: string;
		onCommitClick: (commitId: string) => void;
		onFileClick: (index: number) => void;
		onerror: (err: unknown) => void;
	};

	const {
		projectId,
		stackId,
		inWorkspace,
		isTarget,
		selectedCommitId,
		onCommitClick,
		onFileClick,
		onerror,
	}: Props = $props();

	const stackService = inject(STACK_SERVICE);

	const stackQuery = $derived(stackService.stackById(projectId, stackId));
</script>

<ReduxResult result={stackQuery.result} {projectId} {stackId} {onerror}>
	{#snippet children(stack, { stackId, projectId })}
		{#if stack === null}
			<p>Stack not found.</p>
		{:else}
			{#each stack.segments as segment, idx}
				{@const branchName = segment.refName?.displayName}
				{@const lineColor = getColorFromPushStatus(segment.pushStatus)}

				{#if idx > 0}
					<BranchDividerLine {lineColor} />
				{/if}

				<BranchesViewBranch
					{projectId}
					{stackId}
					{branchName}
					{segment}
					isTopBranch={idx === 0}
					{inWorkspace}
					{isTarget}
					{selectedCommitId}
					{onCommitClick}
					{onFileClick}
					{onerror}
				/>
			{/each}
		{/if}
	{/snippet}
</ReduxResult>

<script lang="ts">
	import StackBranch from '$components/StackBranch.svelte';
	import StackContentPlaceholder from '$components/StackContentPlaceholder.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';

	interface Props {
		stackId: string;
		projectId: string;
	}

	const { stackId, projectId }: Props = $props();

	const stackService = getContext(StackService);
	const result = $derived(stackService.getStackBranches(projectId, stackId));
	const branches = $derived(result.current.data);

	$inspect('getStackBranches', result);
</script>

<div class="stack">
	{#if stackId && branches && branches?.length > 0}
		<div class="stack__branches">
			<div class="stack__branches--list">
				{#each branches as branch}
					<StackBranch {branch} />
				{/each}
			</div>
		</div>

		<div class="stack__branch-content">
			<StackContentPlaceholder />
		</div>
	{/if}
</div>

<style>
	.stack {
		height: 100%;
		flex: 1;
		display: flex;
		border-radius: 0 var(--radius-ml) var(--radius-ml);
	}

	.stack__branches {
		flex: 0.5;
		display: flex;
		flex-direction: column;
		padding: 16px;

		background-color: transparent;
		opacity: 1;
		background-image: radial-gradient(var(--clr-border-2) 0.9px, #ffffff00 0.9px);
		background-size: 12px 12px;
		border-right: 1px solid var(--clr-border-2);
	}

	.stack__branch-content {
		flex: 0.5;
		display: flex;
		flex-direction: column;
	}
</style>

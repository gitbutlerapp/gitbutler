<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import StackContentPlaceholder from '$components/StackContentPlaceholder.svelte';
	import Branch from '$components/v3/Branch.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';

	interface Props {
		stackId: string;
		projectId: string;
	}

	const { stackId, projectId }: Props = $props();

	const stackService = getContext(StackService);
	const result = $derived(stackService.getStackBranches(projectId, stackId));
</script>

<div class="stack">
	<ReduxResult result={result.current}>
		{#snippet children(result)}
			{#if stackId && result.length > 0}
				<div class="stack__branches">
					{#each result as branch, i (branch.name)}
						{@const first = i === 0}
						{@const last = i === result.length - 1}
						<Branch {branch} {first} {last} />
					{/each}
				</div>
			{/if}
		{/snippet}
	</ReduxResult>

	<div class="stack__branch-content">
		<StackContentPlaceholder />
	</div>
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

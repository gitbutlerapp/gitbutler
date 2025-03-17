<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';

	interface Props {
		stackId: string;
		projectId: string;
		branchName?: string;
	}

	const { stackId, projectId, branchName }: Props = $props();

	const [stackService] = inject(StackService);
</script>

{#if branchName}
	{@const branchResult = stackService.branchByName(projectId, stackId, branchName)}
	<ReduxResult result={branchResult.current}>
		{#snippet children(branch)}
			<div class="branch-view">{branch.name}</div>
		{/snippet}
	</ReduxResult>
{/if}

<style>
	.branch-view {
		position: relative;
		height: 100%;
		flex-grow: 1;
		display: flex;
		border-radius: 0 var(--radius-ml) var(--radius-ml);
	}
</style>

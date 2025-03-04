<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import SomethingWentWrong from '$components/SomethingWentWrong.svelte';
	import { branchPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const projectId = $derived(page.params.projectId);
	const stackId = $derived(page.params.stackId);

	const stackService = getContext(StackService);
</script>

{#if projectId && stackId}
	{@const result = stackService.branchAt(projectId, stackId, 0).current}
	<ReduxResult {result}>
		{#snippet children(branch)}
			{#if branch}
				{goto(branchPath(projectId, stackId, branch.name))}
			{:else}
				{@const error = new Error(`No branches found for stack: ${stackId}`)}
				<SomethingWentWrong {error} />
			{/if}
		{/snippet}
	</ReduxResult>
{/if}

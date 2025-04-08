<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { stackPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const projectId = page.params.projectId;
	const stackService = getContext(StackService);
</script>

{#if projectId}
	{@const result = stackService.stackAt(projectId, 0)}
	<ReduxResult {projectId} result={result.current}>
		{#snippet children(stack, env)}
			{#if stack}
				{goto(stackPath(env.projectId, stack.id))}
			{/if}
		{/snippet}
	</ReduxResult>
{/if}

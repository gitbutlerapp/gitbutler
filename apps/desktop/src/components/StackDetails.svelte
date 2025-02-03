<script lang="ts">
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';

	interface Props {
		stackId: string;
		projectId: string;
	}

	const { stackId, projectId }: Props = $props();

	const stackService = getContext(StackService);
	const result = $derived(stackService.getStackBranches(projectId, stackId));

	$inspect('getStackBranches', result);
</script>

<div class="stack">
	{#if stackId}
		<pre>
{JSON.stringify(result.current, null, 2)}
		</pre>
	{/if}
</div>

<style>
	.stack {
		flex: 1;
		border-radius: 0 var(--radius-ml) var(--radius-ml);
	}
	.stack pre {
		text-wrap: wrap;
	}
</style>

<script lang="ts">
	import Branch from './Branch.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId: string;
	};

	const { projectId, stackId }: Props = $props();
	const [stackService] = inject(StackService);

	const result = $derived(stackService.branches(projectId, stackId));
</script>

{#if stackId && result}
	<ReduxResult result={result.current}>
		{#snippet children(branches)}
			{#each branches as branch, i (branch.name)}
				{@const first = i === 0}
				{@const last = i === branches.length - 1}
				<Branch {projectId} {stackId} branchName={branch.name} {first} {last} />
			{/each}
		{/snippet}
	</ReduxResult>
{/if}

<style lang="postcss">
</style>

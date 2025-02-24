<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import CommitView from '$components/v3/CommitView.svelte';
	import StackContentPlaceholder from '$components/v3/StackContentPlaceholder.svelte';
	import { branchPath, commitPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { getContext } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const { projectId, stackId, branchName } = $derived(page.params);
	const upstream = $derived(page.url.searchParams.has('upstream'));
	const commitId = $derived(page.url.searchParams.get('commitId'));
	const stackService = getContext(StackService);
</script>

{#if projectId && stackId && branchName}
	{@const firstLocalAndRemote = stackService.commitAt(projectId, stackId, branchName, 0).current}
	{@const firstUpstream = stackService.upstreamCommitAt(projectId, stackId, branchName, 0).current}

	<ReduxResult result={combineResults(firstLocalAndRemote, firstUpstream)}>
		{#snippet children([firstLocalAndRemote, firstUpstream])}
			{@const firstCommitId = firstLocalAndRemote ? firstLocalAndRemote.id : firstUpstream?.id}
			{@const upstream = !firstLocalAndRemote && !!firstUpstream}
			{#if !commitId && firstCommitId}
				{goto(
					commitPath(projectId, {
						stackId,
						branchName,
						commitId: firstCommitId,
						upstream
					})
				)}
			{/if}
		{/snippet}
	</ReduxResult>

	{#if commitId}
		<CommitView
			{projectId}
			commitKey={{ stackId, branchName, commitId, upstream }}
			onClose={() => goto(branchPath(projectId, stackId, branchName))}
		/>
	{:else}
		<StackContentPlaceholder isNewStack={true} />
	{/if}
{/if}

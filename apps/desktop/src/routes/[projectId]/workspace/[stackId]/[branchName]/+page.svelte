<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import CommitView from '$components/v3/CommitView.svelte';
	import StackContentIllustration, {
		PreviewMode
	} from '$components/v3/StackContentIllustration.svelte';
	import { commitPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineQueries } from '$lib/state/helpers';
	import { getContext } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const { projectId, stackId, branchName } = $derived(page.params);
	const upstream = $derived(page.url.searchParams.has('upstream'));
	const commitId = $derived(page.url.searchParams.get('commitId'));
	const stackService = getContext(StackService);
</script>

{#if projectId && stackId && branchName}
	{@const firstLocalAndRemote = stackService.commits(projectId, stackId, branchName, {
		index: 0
	}).current}
	{@const firstUpstream = stackService.upstreamCommits(projectId, stackId, branchName, {
		index: 0
	}).current}

	<ReduxResult result={combineQueries(firstLocalAndRemote, firstUpstream)}>
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
			{stackId}
			{branchName}
			commitKey={{ stackId, branchName, commitId, upstream }}
		/>
	{:else}
		<StackContentIllustration mode={PreviewMode.NewStack} />
	{/if}
{/if}

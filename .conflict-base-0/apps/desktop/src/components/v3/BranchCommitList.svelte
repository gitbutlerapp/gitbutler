<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { inject } from '@gitbutler/shared/context';
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';
	import type { CommitKey } from '$lib/commits/commit';
	import type { Snippet } from 'svelte';

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
		selectedCommitId?: string;
		empty?: Snippet;
		upstreamTemplate?: Snippet<
			[
				{
					commit: UpstreamCommit;
					commitKey: CommitKey;
					first: boolean;
					lastCommit: boolean;
					selected: boolean;
				}
			]
		>;
		localAndRemoteTemplate?: Snippet<
			[
				{
					commit: Commit;
					commitKey: CommitKey;
					first: boolean;
					last: boolean;
					lastCommit: boolean;
					selectedCommitId: string | undefined;
				}
			]
		>;
	}

	let {
		projectId,
		stackId,
		branchName,
		selectedCommitId,
		empty,
		localAndRemoteTemplate,
		upstreamTemplate
	}: Props = $props();

	const [stackService] = inject(StackService);

	const localAndRemoteCommits = $derived(stackService.commits(projectId, stackId, branchName));
	const upstreamOnlyCommits = $derived(
		stackService.upstreamCommits(projectId, stackId, branchName)
	);
</script>

<ReduxResult
	{stackId}
	{projectId}
	result={combineResults(upstreamOnlyCommits.current, localAndRemoteCommits.current)}
>
	{#snippet children([upstreamOnlyCommits, localAndRemoteCommits], { stackId })}
		{#if localAndRemoteCommits.length === 0}
			{@render empty?.()}
		{/if}
		<div class="commit-list">
			{#if upstreamTemplate}
				{#each upstreamOnlyCommits as commit, i (commit.id)}
					{@const first = i === 0}
					{@const lastCommit = i === upstreamOnlyCommits.length - 1}
					{@const commitKey = { stackId, branchName, commitId: commit.id, upstream: true }}
					{@const selected = selectedCommitId === commit.id}
					{@render upstreamTemplate({ commit, commitKey, first, lastCommit, selected })}
				{/each}
			{/if}

			{#if localAndRemoteTemplate}
				{#each localAndRemoteCommits as commit, i (commit.id)}
					{@const first = i === 0}
					{@const last = i === localAndRemoteCommits.length - 1}
					{@const commitKey = { stackId, branchName, commitId: commit.id, upstream: false }}
					{@render localAndRemoteTemplate({
						commit,
						commitKey,
						first,
						last,
						lastCommit: last,
						selectedCommitId
					})}
				{/each}
			{/if}
		</div>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.commit-list {
		position: relative;
		display: flex;
		flex-direction: column;
		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
	}
</style>

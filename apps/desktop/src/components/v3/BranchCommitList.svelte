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
		lastBranch?: boolean;
		selectedBranchName?: string;
		selectedCommitId?: string;
		upstreamTemplate?: Snippet<
			[
				{
					commit: UpstreamCommit;
					commitKey: CommitKey;
					first: boolean;
					last: boolean;
					selected: boolean;
				}
			]
		>;
		localAndRemoteTemplate?: Snippet<
			[{ commit: Commit; commitKey: CommitKey; first: boolean; last: boolean; selected: boolean }]
		>;
		empty?: Snippet;
	}

	let {
		projectId,
		stackId,
		branchName,
		selectedCommitId,
		localAndRemoteTemplate,
		upstreamTemplate,
		empty
	}: Props = $props();

	const [stackService] = inject(StackService);

	const localAndRemoteCommits = $derived(
		stackService.commits(projectId, stackId, branchName).current
	);
	const upstreamOnlyCommits = $derived(
		stackService.upstreamCommits(projectId, stackId, branchName).current
	);
</script>

<ReduxResult result={combineResults(upstreamOnlyCommits, localAndRemoteCommits)}>
	{#snippet children([upstreamOnlyCommits, localAndRemoteCommits])}
		{#if !upstreamOnlyCommits.length && !localAndRemoteCommits.length}
			{@render empty?.()}
		{:else}
			<div class="commit-list">
				{#if upstreamTemplate}
					{#each upstreamOnlyCommits as commit, i (commit.id)}
						{@const first = i === 0}
						{@const last = i === upstreamOnlyCommits.length - 1}
						{@const commitKey = { stackId, branchName, commitId: commit.id, upstream: true }}
						{@const selected = selectedCommitId === commit.id}
						{@render upstreamTemplate({ commit, commitKey, first, last, selected })}
					{/each}
				{/if}

				{#if localAndRemoteTemplate}
					{#each localAndRemoteCommits as commit, i (commit.id)}
						{@const first = i === 0}
						{@const last = i === localAndRemoteCommits.length - 1}
						{@const commitKey = { stackId, branchName, commitId: commit.id, upstream: false }}
						{@const selected = selectedCommitId === commit.id}
						{@render localAndRemoteTemplate({ commit, commitKey, first, last, selected })}
					{/each}
				{/if}
			</div>
		{/if}
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

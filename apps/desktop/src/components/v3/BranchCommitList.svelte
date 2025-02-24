<script lang="ts">
	import EmptyBranch from './EmptyBranch.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import CommitRow from '$components/v3/CommitRow.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { inject } from '@gitbutler/shared/context';

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
		lastBranch?: boolean;
		selectedCommitId?: string;
	}

	let { projectId, stackId, branchName, lastBranch, selectedCommitId }: Props = $props();

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
			<EmptyBranch {lastBranch} />
		{:else}
			<div class="commit-list">
				{#each upstreamOnlyCommits as commit, i (commit.id)}
					{@const first = i === 0}
					{@const last = i === upstreamOnlyCommits.length - 1}
					{@const commitKey = { stackId, branchName, commitId: commit.id, upstream: true }}
					{@const selected = selectedCommitId === commit.id}
					<CommitRow {projectId} {commitKey} {first} {last} {commit} {selected} />
				{/each}

				{#each localAndRemoteCommits as commit, i (commit.id)}
					{@const first = i === 0}
					{@const last = i === localAndRemoteCommits.length - 1}
					{@const commitKey = { stackId, branchName, commitId: commit.id, upstream: false }}
					{@const selected = selectedCommitId === commit.id}
					<CommitRow {projectId} {commitKey} {first} {last} {commit} {lastBranch} {selected} />
				{/each}
			</div>
		{/if}
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.commit-list {
		position: relative;
		display: flex;
		flex-direction: column;
		border-radius: 0 0 var(--radius-m) var(--radius-m);
	}
</style>

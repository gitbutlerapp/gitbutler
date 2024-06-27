<script lang="ts">
	import CommitCard from './CommitCard.svelte';
	import { filterCommits, type AppliedFilter } from '$lib/vbranches/filtering';
	import type { CommitStatus, RemoteCommit } from '$lib/vbranches/types';
	import type { Snippet } from 'svelte';

	interface Props {
		commits: RemoteCommit[];
		isUnapplied: boolean;
		type: CommitStatus;
		searchQuery: string | undefined;
		searchFilters: AppliedFilter[];
		getCommitUrl: (commitId: string) => string | undefined;
		onAuthorClick?: (author: string) => void;
		onFileClick?: (filePath: string) => void;
		children?: Snippet;
	}

	let {
		commits,
		isUnapplied,
		type,
		getCommitUrl,
		onAuthorClick,
		onFileClick,
		searchFilters,
		searchQuery,
		children
	}: Props = $props();

	let filteredCommits = $derived<RemoteCommit[]>(
		filterCommits(commits, searchQuery, searchFilters, type)
	);

	export function isEmpty() {
		return filteredCommits.length === 0;
	}
</script>

{#if !isEmpty()}
	<div>
		{#if children}
			{@render children()}
		{/if}
		{#each filteredCommits as commit, index (commit.id)}
			<CommitCard
				{commit}
				first={index === 0}
				last={index === filteredCommits.length - 1}
				{isUnapplied}
				commitUrl={getCommitUrl(commit.id)}
				{type}
				{onAuthorClick}
				{onFileClick}
			/>
		{/each}
	</div>
{/if}

<script lang="ts">
	import CommitCard from './CommitCard.svelte';
	import { getFilterContext } from '$lib/searchBar/filterContext.svelte';
	import {  FilterName, filterCommits } from '$lib/vbranches/filtering';
	import type { CommitStatus, RemoteCommit } from '$lib/vbranches/types';
	import type { Snippet } from 'svelte';

	interface Props {
		commits: RemoteCommit[];
		isUnapplied: boolean;
		type: CommitStatus;
		getCommitUrl: (commitId: string) => string | undefined;
		children?: Snippet;
	}

	let { commits, isUnapplied, type, getCommitUrl, children }: Props = $props();

	const filterContext = getFilterContext();

	let filteredCommits = $derived<RemoteCommit[]>(
		filterCommits(commits, filterContext.searchQuery, filterContext.appliedFilters, type)
	);

	export function isEmpty() {
		return filteredCommits.length === 0;
	}

	function onAuthorClick(author: string) {
		filterContext.addFilter({ name: FilterName.Author, values: [author] });
	}

	function onFileClick(file: string) {
		filterContext.addFilter({ name: FilterName.File, values: [file] });
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

<script lang="ts">
	import CommitList from './CommitList.svelte';
	import type { Branch, AnyFile, RemoteBranchData } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let branch: Branch;
	export let selectedFiles: Writable<AnyFile[]>;
	export let isUnapplied: boolean;
	export let branchCount: number;
	export let remoteBranchData: RemoteBranchData | undefined;

	$: unknownCommits = remoteBranchData?.commits.filter(
		(remoteCommit) => !branch.commits.find((commit) => remoteCommit.id == commit.id)
	);
</script>

{#if unknownCommits && unknownCommits.length > 0}
	<CommitList
		{branch}
		{branchCount}
		{isUnapplied}
		{selectedFiles}
		commits={unknownCommits}
		type="upstream"
	/>
{/if}
<CommitList
	{branch}
	{isUnapplied}
	{selectedFiles}
	commits={branch.commits.filter((c) => c.status == 'local')}
	type="local"
/>
<CommitList
	{branch}
	{isUnapplied}
	{selectedFiles}
	type="remote"
	commits={branch.commits.filter((c) => c.status == 'remote')}
/>
<CommitList
	{branch}
	{isUnapplied}
	{selectedFiles}
	type="integrated"
	commits={branch.commits.filter((c) => c.status == 'integrated')}
/>

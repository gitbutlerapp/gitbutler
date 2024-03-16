<script lang="ts">
	import CommitList from './CommitList.svelte';
	import type { Project } from '$lib/backend/projects';
	import type { Branch, AnyFile, RemoteBranchData } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let project: Project;
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
		{project}
		{branchCount}
		{isUnapplied}
		{selectedFiles}
		commits={unknownCommits}
		type="upstream"
	/>
{/if}
<CommitList
	{branch}
	{project}
	{isUnapplied}
	{selectedFiles}
	commits={branch.commits.filter((c) => c.status == 'local')}
	type="local"
/>
<CommitList
	{branch}
	{project}
	{isUnapplied}
	{selectedFiles}
	type="remote"
	commits={branch.commits.filter((c) => c.status == 'remote')}
/>
<CommitList
	{branch}
	{project}
	{isUnapplied}
	{selectedFiles}
	type="integrated"
	commits={branch.commits.filter((c) => c.status == 'integrated')}
/>

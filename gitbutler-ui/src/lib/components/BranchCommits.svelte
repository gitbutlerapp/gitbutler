<script lang="ts">
	import CommitList from './CommitList.svelte';
	import type { Project } from '$lib/backend/projects';
	import type { BranchService } from '$lib/branches/service';
	import type { GitHubService } from '$lib/github/service';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch, AnyFile, RemoteBranchData } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let project: Project;
	export let branch: Branch;
	export let base: BaseBranch | undefined | null;
	export let githubService: GitHubService;
	export let branchController: BranchController;
	export let branchService: BranchService;
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
		{base}
		{project}
		{branchController}
		{branchService}
		{branchCount}
		{githubService}
		{isUnapplied}
		{selectedFiles}
		commits={unknownCommits}
		type="upstream"
	/>
{/if}
<CommitList
	{branch}
	{base}
	{project}
	{branchController}
	{branchService}
	{githubService}
	{isUnapplied}
	{selectedFiles}
	commits={branch.commits.filter((c) => c.status == 'local')}
	type="local"
/>
<CommitList
	{branch}
	{base}
	{project}
	{branchController}
	{branchService}
	{githubService}
	{isUnapplied}
	{selectedFiles}
	type="remote"
	commits={branch.commits.filter((c) => c.status == 'remote')}
/>
<CommitList
	{branch}
	{base}
	{project}
	{branchController}
	{branchService}
	{githubService}
	{isUnapplied}
	{selectedFiles}
	type="integrated"
	commits={branch.commits.filter((c) => c.status == 'integrated')}
/>

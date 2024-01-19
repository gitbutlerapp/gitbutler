<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import type { GitHubService } from '$lib/github/service';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import CommitList from './CommitList.svelte';

	export let project: Project;
	export let branch: Branch;
	export let base: BaseBranch | undefined | null;
	export let githubService: GitHubService;
	export let branchController: BranchController;
	export let readonly: boolean;
	export let branchCount: number;
</script>

{#if branch.commits.length > 0 || (branch.upstream && branch.upstream.commits.length > 0)}
	<CommitList
		{branch}
		{base}
		{project}
		{branchController}
		{branchCount}
		{githubService}
		{readonly}
		type="upstream"
	/>
	<CommitList
		{branch}
		{base}
		{project}
		{branchController}
		{githubService}
		{readonly}
		type="local"
	/>
	<CommitList
		{branch}
		{base}
		{project}
		{branchController}
		{githubService}
		{readonly}
		type="remote"
	/>
	<CommitList
		{branch}
		{base}
		{project}
		{branchController}
		{githubService}
		{readonly}
		type="integrated"
	/>
{/if}

<script lang="ts">
	import CommitList from './CommitList.svelte';
	import type { Project } from '$lib/backend/projects';
	import type { BranchService } from '$lib/branches/service';
	import type { GitHubService } from '$lib/github/service';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch, AnyFile } from '$lib/vbranches/types';
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
</script>

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
    type="upstream"
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
/>

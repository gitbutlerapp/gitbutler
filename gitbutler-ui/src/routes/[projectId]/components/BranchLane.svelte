<script lang="ts">
	import type { BaseBranch, Branch, File } from '$lib/vbranches/types';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { User, getCloudApiClient } from '$lib/backend/cloud';
	import BranchCard from './BranchCard.svelte';
	import FileCard from './FileCard.svelte';
	import { writable, type Writable } from 'svelte/store';
	import { Ownership } from '$lib/vbranches/ownership';
	import type { GitHubService } from '$lib/github/service';
	import type { Project } from '$lib/backend/projects';

	export let branch: Branch;
	export let readonly = false;
	export let project: Project;
	export let base: BaseBranch | undefined | null;
	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let branchController: BranchController;
	export let maximized = false;
	export let branchCount = 1;
	export let user: User | undefined;
	export let projectPath: string;
	export let githubService: GitHubService;

	$: selectedOwnership = writable(Ownership.fromBranch(branch));
	$: selected = setSelected($selectedFiles, branch);

	const selectedFiles = writable<File[]>([]);

	let commitBoxOpen: Writable<boolean>;

	function setSelected(files: File[], branch: Branch) {
		if (files.length == 0) return undefined;
		const match = branch.files?.find((f) => files[0].id == f.id);
		if (!match) $selectedFiles = [];
		return match;
	}
</script>

<div class="wrapper card">
	<BranchCard
		{branch}
		{readonly}
		{project}
		{base}
		{cloud}
		{branchController}
		{selectedOwnership}
		bind:commitBoxOpen
		{maximized}
		{branchCount}
		{user}
		{selectedFiles}
		{githubService}
	/>

	{#if selected}
		<FileCard
			conflicted={selected.conflicted}
			branchId={branch.id}
			file={selected}
			projectId={project.id}
			{projectPath}
			{branchController}
			{selectedOwnership}
			selectable={$commitBoxOpen && !readonly}
			on:close={() => {
				const selectedId = selected?.id;
				selectedFiles.update((fileIds) => fileIds.filter((file) => file.id != selectedId));
			}}
		/>
	{/if}
</div>

<style lang="postcss">
	.wrapper {
		height: 100%;
		flex-shrink: 0;
	}

	.card {
		flex-direction: row;
	}
</style>

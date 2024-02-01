<script lang="ts">
	import BranchCard from './BranchCard.svelte';
	import FileCard from './FileCard.svelte';
	import { Ownership } from '$lib/vbranches/ownership';
	import { RemoteFile, type BaseBranch, type Branch, type LocalFile } from '$lib/vbranches/types';
	import { writable, type Writable } from 'svelte/store';
	import type { User, getCloudApiClient } from '$lib/backend/cloud';
	import type { Project } from '$lib/backend/projects';
	import type { BranchService } from '$lib/branches/service';
	import type { GitHubService } from '$lib/github/service';
	import type { BranchController } from '$lib/vbranches/branchController';

	export let branch: Branch;
	export let isUnapplied = false;
	export let project: Project;
	export let base: BaseBranch | undefined | null;
	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let branchController: BranchController;
	export let branchService: BranchService;
	export let branchCount = 1;
	export let user: User | undefined;
	export let projectPath: string;
	export let githubService: GitHubService;

	$: selectedOwnership = writable(Ownership.fromBranch(branch));
	$: selected = setSelected($selectedFiles, branch);

	const selectedFiles = writable<LocalFile[]>([]);

	let commitBoxOpen: Writable<boolean>;

	function setSelected(files: (LocalFile | RemoteFile)[], branch: Branch) {
		if (files.length == 0) return undefined;
		if (files.length == 1 && files[0] instanceof RemoteFile) return files[0];

		// If regular file selected but not found in branch files then clear selection.
		const match = branch.files?.find((f) => files[0].id == f.id);
		if (!match) $selectedFiles = [];
		return match;
	}
</script>

<div
	class="wrapper"
	data-tauri-drag-region
	class:target-branch={branch.selectedForChanges}
	class:selected
>
	<BranchCard
		{branch}
		{isUnapplied}
		{project}
		{base}
		{cloud}
		{branchController}
		{branchService}
		{selectedOwnership}
		bind:commitBoxOpen
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
			{isUnapplied}
			selectable={$commitBoxOpen && !isUnapplied}
			on:close={() => {
				const selectedId = selected?.id;
				selectedFiles.update((fileIds) => fileIds.filter((file) => file.id != selectedId));
			}}
		/>
	{/if}
</div>

<style lang="postcss">
	.wrapper {
		display: flex;
		height: 100%;
		align-items: self-start;
		flex-shrink: 0;
		position: relative;
		--target-branch-background: var(--clr-theme-container-pale);
		--selected-resize-shift: 0;
		--selected-opacity: 1;
		background-color: var(--target-branch-background);
	}

	.target-branch {
		--target-branch-background: color-mix(
			in srgb,
			var(--clr-theme-scale-pop-60) 15%,
			var(--clr-theme-container-pale)
		);
	}

	.selected {
		--selected-resize-shift: calc(var(--space-4) * -1);
		--selected-opacity: 0;
	}
</style>

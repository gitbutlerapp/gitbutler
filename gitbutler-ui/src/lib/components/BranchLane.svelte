<script lang="ts">
	import BranchCard from './BranchCard.svelte';
	import FileCard from './FileCard.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import { projectLaneCollapsed } from '$lib/config/config';
	import { persisted } from '$lib/persisted/persisted';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { Ownership } from '$lib/vbranches/ownership';
	import {
		RemoteFile,
		type BaseBranch,
		type Branch,
		type LocalFile,
		type AnyFile
	} from '$lib/vbranches/types';
	import lscache from 'lscache';
	import { getContext } from 'svelte';
	import { quintOut } from 'svelte/easing';
	import { writable } from 'svelte/store';
	import { slide } from 'svelte/transition';
	import type { User, getCloudApiClient } from '$lib/backend/cloud';
	import type { Project } from '$lib/backend/projects';

	export let branch: Branch;
	export let isUnapplied = false;
	export let project: Project;
	export let base: BaseBranch | undefined | null;
	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let branchCount = 1;
	export let user: User | undefined;
	export let projectPath: string;

	$: selectedOwnership = writable(Ownership.fromBranch(branch));
	$: selected = setSelected($selectedFiles, branch);

	const selectedFiles = writable<LocalFile[]>([]);

	let rsViewport: HTMLElement;

	const commitBoxOpen = persisted<boolean>(false, 'commitBoxExpanded_' + branch.id);
	const defaultFileWidthRem = persisted<number | undefined>(30, 'defaulFileWidth' + project.id);
	const fileWidthKey = 'fileWidth_';
	let fileWidth: number;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	fileWidth = lscache.get(fileWidthKey + branch.id);

	function setSelected(files: AnyFile[], branch: Branch) {
		if (files.length == 0) return undefined;
		if (files.length == 1 && files[0] instanceof RemoteFile) return files[0];

		// If regular file selected but not found in branch files then clear selection.
		const match = branch.files?.find((f) => files[0].id == f.id);
		if (!match) $selectedFiles = [];
		return match;
	}

	$: isLaneCollapsed = projectLaneCollapsed(project.id, branch.id);
	$: if ($isLaneCollapsed) {
		$selectedFiles = [];
	}
</script>

<div
	class="wrapper"
	data-tauri-drag-region
	class:target-branch={branch.active && branch.selectedForChanges}
	class:file-selected={selected}
>
	<BranchCard
		{branch}
		{isUnapplied}
		{project}
		{base}
		{cloud}
		{selectedOwnership}
		{commitBoxOpen}
		bind:isLaneCollapsed
		{branchCount}
		{user}
		{selectedFiles}
	/>

	{#if selected}
		<div
			class="file-preview resize-viewport"
			bind:this={rsViewport}
			in:slide={{ duration: 180, easing: quintOut, axis: 'x' }}
			style:width={`${fileWidth || $defaultFileWidthRem}rem`}
		>
			<FileCard
				conflicted={selected.conflicted}
				branchId={branch.id}
				file={selected}
				{projectPath}
				{selectedOwnership}
				{isUnapplied}
				branchCommits={branch.commits}
				readonly={selected instanceof RemoteFile}
				selectable={$commitBoxOpen && !isUnapplied}
				on:close={() => {
					const selectedId = selected?.id;
					selectedFiles.update((fileIds) => fileIds.filter((file) => file.id != selectedId));
				}}
			/>
			<Resizer
				viewport={rsViewport}
				direction="right"
				minWidth={240}
				defaultLineColor="var(--clr-theme-container-outline-light)"
				on:width={(e) => {
					fileWidth = e.detail / (16 * $userSettings.zoom);
					lscache.set(fileWidthKey + branch.id, fileWidth, 7 * 1440); // 7 day ttl
					$defaultFileWidthRem = fileWidth;
				}}
			/>
		</div>
	{/if}
</div>

<style lang="postcss">
	.wrapper {
		display: flex;
		height: 100%;
		align-items: self-start;
		flex-shrink: 0;
		user-select: none; /* here because of user-select draggable interference in board */
		position: relative;
		--target-branch-background: var(--clr-theme-container-pale);
		background-color: var(--target-branch-background);
	}

	.target-branch {
		--target-branch-background: color-mix(
			in srgb,
			var(--clr-theme-scale-pop-60) 20%,
			var(--clr-theme-container-pale)
		);
	}

	.file-preview {
		display: flex;
		position: relative;
		height: 100%;

		overflow: hidden;
		align-items: self-start;

		padding: var(--space-12) var(--space-12) var(--space-12) 0;
	}
</style>

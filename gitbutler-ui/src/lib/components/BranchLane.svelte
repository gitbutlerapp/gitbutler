<script lang="ts">
	import BranchCard from './BranchCard.svelte';
	import FileCard from './FileCard.svelte';
	import { Project } from '$lib/backend/projects';
	import Resizer from '$lib/components/Resizer.svelte';
	import { projectLaneCollapsed } from '$lib/config/config';
	import { persisted } from '$lib/persisted/persisted';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getRemoteBranchData } from '$lib/stores/remoteBranches';
	import { getContext, getContextStoreBySymbol, createContextStore } from '$lib/utils/context';
	import {
		createIntegratedContextStore,
		createLocalContextStore,
		createRemoteContextStore,
		createUnknownContextStore
	} from '$lib/vbranches/contexts';
	import { Ownership } from '$lib/vbranches/ownership';
	import { RemoteFile, Branch, type LocalFile, type AnyFile } from '$lib/vbranches/types';
	import lscache from 'lscache';
	import { quintOut } from 'svelte/easing';
	import { writable } from 'svelte/store';
	import { slide } from 'svelte/transition';

	export let branch: Branch;
	export let isUnapplied = false;

	$: selected = setSelected($selectedFiles, branch);

	const ownershipStore = createContextStore(Ownership, Ownership.fromBranch(branch));
	// TODO: Update store here rather than reset it
	$: ownershipStore.set(Ownership.fromBranch(branch));

	const branchStore = createContextStore(Branch, branch);
	$: branchStore.set(branch);

	const localCommits = createLocalContextStore(branch.localCommits);
	$: localCommits.set(branch.localCommits);

	const remoteCommits = createRemoteContextStore(branch.remoteCommits);
	$: remoteCommits.set(branch.remoteCommits);

	const integratedCommits = createIntegratedContextStore(branch.integratedCommits);
	$: integratedCommits.set(branch.integratedCommits);

	// Set the store immediately so it can be updated later.
	const unknownCommits = createUnknownContextStore([]);
	$: if (branch.upstream?.name) loadRemoteBranch(branch.upstream?.name);

	async function loadRemoteBranch(name: string) {
		const remoteBranchData = await getRemoteBranchData(project.id, name);
		const commits = remoteBranchData?.commits.filter(
			(remoteCommit) => !branch.commits.find((commit) => remoteCommit.id == commit.id)
		);
		unknownCommits.set(commits);
	}

	const project = getContext(Project);
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const selectedFiles = writable<LocalFile[]>([]);

	let rsViewport: HTMLElement;

	const commitBoxOpen = persisted<boolean>(false, 'commitBoxExpanded_' + branch.id);
	const defaultFileWidthRem = persisted<number | undefined>(30, 'defaulFileWidth' + project.id);
	const fileWidthKey = 'fileWidth_';
	let fileWidth: number;

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
	<BranchCard {isUnapplied} {commitBoxOpen} bind:isLaneCollapsed {selectedFiles} />

	{#if selected}
		<div
			class="file-preview resize-viewport"
			bind:this={rsViewport}
			in:slide={{ duration: 180, easing: quintOut, axis: 'x' }}
			style:width={`${fileWidth || $defaultFileWidthRem}rem`}
		>
			<FileCard
				conflicted={selected.conflicted}
				file={selected}
				{isUnapplied}
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
				defaultLineColor="var(--clr-container-outline-light)"
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
		--target-branch-background: var(--clr-container-pale);
		background-color: var(--target-branch-background);
	}

	.target-branch {
		--target-branch-background: color-mix(
			in srgb,
			var(--clr-scale-pop-60) 20%,
			var(--clr-container-pale)
		);
	}

	.file-preview {
		display: flex;
		position: relative;
		height: 100%;

		overflow: hidden;
		align-items: self-start;

		padding: var(--size-12) var(--size-12) var(--size-12) 0;
	}
</style>

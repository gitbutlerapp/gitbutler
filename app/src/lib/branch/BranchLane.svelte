<script lang="ts">
	import BranchCard from './BranchCard.svelte';
	import { Project } from '$lib/backend/projects';
	import Resizer from '$lib/components/Resizer.svelte';
	import { projectLaneCollapsed } from '$lib/config/config';
	import FileCard from '$lib/file/FileCard.svelte';
	import { persisted } from '$lib/persisted/persisted';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContext, getContextStoreBySymbol, createContextStore } from '$lib/utils/context';
	import {
		createIntegratedContextStore,
		createLocalContextStore,
		createRemoteContextStore,
		createUnknownCommitsStore,
		createUpstreamContextStore
	} from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { Ownership } from '$lib/vbranches/ownership';
	import { RemoteFile, Branch } from '$lib/vbranches/types';
	import lscache from 'lscache';
	import { setContext } from 'svelte';
	import { quintOut } from 'svelte/easing';
	import { writable } from 'svelte/store';
	import { slide } from 'svelte/transition';

	export let branch: Branch;
	export let isUnapplied = false;

	const ownershipStore = createContextStore(Ownership, Ownership.fromBranch(branch));
	// TODO: Update store here rather than reset it
	$: ownershipStore.set(Ownership.fromBranch(branch));

	const branchStore = createContextStore(Branch, undefined);
	$: branchStore.set(branch);

	const localCommits = createLocalContextStore(undefined);
	$: localCommits.set(branch.localCommits);

	const remoteCommits = createRemoteContextStore(undefined);
	$: remoteCommits.set(branch.remoteCommits);

	// Set the store immediately so it can be updated later.
	const upstreamCommits = createUpstreamContextStore([]);
	$: upstreamCommits.set(branch.upstreamData?.commits ?? []);

	const unknownCommits = createUnknownCommitsStore([]);
	$: unknownCommits.set($upstreamCommits.filter((c) => !c.relatedTo));

	const integratedCommits = createIntegratedContextStore([]);
	$: integratedCommits.set(branch.integratedCommits);

	const project = getContext(Project);

	const branchFiles = writable(branch.files);
	$: branchFiles.set(branch.files);
	const fileIdSelection = new FileIdSelection(project.id, branchFiles);
	setContext(FileIdSelection, fileIdSelection);

	$: selectedFile = fileIdSelection.selectedFile;

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	let rsViewport: HTMLElement;

	const commitBoxOpen = persisted<boolean>(false, 'commitBoxExpanded_' + branch.id);
	const defaultFileWidthRem = persisted<number | undefined>(30, 'defaulFileWidth' + project.id);
	const fileWidthKey = 'fileWidth_';
	let fileWidth: number;

	fileWidth = lscache.get(fileWidthKey + branch.id);

	$: isLaneCollapsed = projectLaneCollapsed(project.id, branch.id);
	$: if ($isLaneCollapsed) {
		fileIdSelection.clear();
	}
</script>

<div class="wrapper" data-tauri-drag-region>
	<BranchCard {isUnapplied} {commitBoxOpen} bind:isLaneCollapsed />

	{#await $selectedFile then selected}
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
						fileIdSelection.clear();
					}}
				/>
				<Resizer
					viewport={rsViewport}
					direction="right"
					minWidth={400}
					defaultLineColor="var(--clr-border-2)"
					on:width={(e) => {
						fileWidth = e.detail / (16 * $userSettings.zoom);
						lscache.set(fileWidthKey + branch.id, fileWidth, 7 * 1440); // 7 day ttl
						$defaultFileWidthRem = fileWidth;
					}}
				/>
			</div>
		{/if}
	{/await}
</div>

<style lang="postcss">
	.wrapper {
		display: flex;
		height: 100%;
		align-items: self-start;
		flex-shrink: 0;
		user-select: none; /* here because of user-select draggable interference in board */
		position: relative;
	}

	.file-preview {
		display: flex;
		position: relative;
		height: 100%;

		overflow: hidden;
		align-items: self-start;

		padding: 12px 12px 12px 0;
	}
</style>

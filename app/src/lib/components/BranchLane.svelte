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
	import { isDefined } from '$lib/utils/typeguards';
	import {
		createLocalContextStore,
		createRemoteContextStore,
		createSelectedFiles,
		createUnknownContextStore
	} from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { Ownership } from '$lib/vbranches/ownership';
	import { RemoteFile, Branch, commitCompare, RemoteCommit } from '$lib/vbranches/types';
	import lscache from 'lscache';
	import { setContext } from 'svelte';
	import { quintOut } from 'svelte/easing';
	import { slide } from 'svelte/transition';

	export let branch: Branch;
	export let isUnapplied = false;

	const ownershipStore = createContextStore(Ownership, Ownership.fromBranch(branch));
	// TODO: Update store here rather than reset it
	$: ownershipStore.set(Ownership.fromBranch(branch));

	const branchStore = createContextStore(Branch, branch);
	$: branchStore.set(branch);

	const localCommits = createLocalContextStore(branch.localCommits);
	$: localCommits.set(branch.localCommits);

	const remoteCommits = createRemoteContextStore(branch.remoteCommits);
	$: remoteCommits.set(branch.remoteCommits);

	// Set the store immediately so it can be updated later.
	const upstreamCommits = createUnknownContextStore([]);
	$: if (branch.upstream?.name) loadRemoteBranch(branch.upstream?.name);

	const fileIdSelection = new FileIdSelection();
	setContext(FileIdSelection, fileIdSelection);

	const selectedFiles = createSelectedFiles([]);
	$: if ($fileIdSelection.length == 0) selectedFiles.set([]);
	$: if ($fileIdSelection.length > 0 && fileIdSelection.only().commitId == 'undefined') {
		selectedFiles.set(
			$fileIdSelection
				.map((fileId) => branch.files.find((f) => f.id + '|' + undefined == fileId))
				.filter(isDefined)
		);
	}

	$: displayedFile = $selectedFiles.length == 1 ? $selectedFiles[0] : undefined;

	async function loadRemoteBranch(name: string) {
		const upstream = await getRemoteBranchData(project.id, name);
		if (!upstream.commits) return;
		const unknownCommits: RemoteCommit[] = [];
		upstream?.commits.forEach((upstreamCommit) => {
			let match = branch.commits.find((commit) => commitCompare(upstreamCommit, commit));
			if (match) {
				match.relatedTo = upstreamCommit;
			} else unknownCommits.push(upstreamCommit);
		});
		if (upstream.commits.length != unknownCommits.length) {
			// Force update since we've mutated local commits
			localCommits.set($localCommits);
		}
		upstreamCommits.set(unknownCommits);
	}

	const project = getContext(Project);
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

<div class="wrapper" data-tauri-drag-region class:file-selected={displayedFile}>
	<BranchCard {isUnapplied} {commitBoxOpen} bind:isLaneCollapsed />

	{#if displayedFile}
		<div
			class="file-preview resize-viewport"
			bind:this={rsViewport}
			in:slide={{ duration: 180, easing: quintOut, axis: 'x' }}
			style:width={`${fileWidth || $defaultFileWidthRem}rem`}
		>
			<FileCard
				conflicted={displayedFile.conflicted}
				file={displayedFile}
				{isUnapplied}
				readonly={displayedFile instanceof RemoteFile}
				selectable={$commitBoxOpen && !isUnapplied}
				on:close={() => {
					fileIdSelection.clear();
				}}
			/>
			<Resizer
				viewport={rsViewport}
				direction="right"
				minWidth={240}
				defaultLineColor="var(--clr-border-2)"
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
		--target-branch-background: var(--clr-bg-2);
		background-color: var(--target-branch-background);
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

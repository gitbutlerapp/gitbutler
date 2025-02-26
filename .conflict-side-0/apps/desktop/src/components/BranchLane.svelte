<script lang="ts">
	import FileCard from '$components/FileCard.svelte';
	import Resizer from '$components/Resizer.svelte';
	import Stack from '$components/v3/Stack.svelte';
	import { BranchStack } from '$lib/branches/branch';
	import { SelectedOwnership } from '$lib/branches/ownership';
	import { projectLaneCollapsed } from '$lib/config/config';
	import { RemoteFile } from '$lib/files/file';
	import { Project } from '$lib/project/project';
	import { FileIdSelection } from '$lib/selection/fileIdSelection';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import {
		getContext,
		getContextStoreBySymbol,
		createContextStore
	} from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import lscache from 'lscache';
	import { setContext } from 'svelte';
	import { quintOut } from 'svelte/easing';
	import { writable } from 'svelte/store';
	import { slide } from 'svelte/transition';

	const { stack: branch }: { stack: BranchStack } = $props();

	// BRANCH
	const branchStore = createContextStore(BranchStack, branch);
	const selectedOwnershipStore = createContextStore(
		SelectedOwnership,
		SelectedOwnership.fromBranch(branch)
	);
	const uncommittedFiles = writable(branch.files);

	$effect(() => {
		branchStore.set(branch);
		selectedOwnershipStore.update((o) => o?.update(branch));
		uncommittedFiles.set(branch.files);
	});

	const project = getContext(Project);

	const fileIdSelection = new FileIdSelection();
	const selectedFile = fileIdSelection.selectedFile;
	const commitId = $derived($selectedFile?.commitId);
	const selected = $derived($selectedFile?.file);
	setContext(FileIdSelection, fileIdSelection);
	$effect(() => {
		fileIdSelection.setUncommittedFiles($uncommittedFiles);
	});

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	let rsViewport: HTMLElement | undefined = $state();

	const commitBoxOpen = persisted<boolean>(false, 'commitBoxExpanded_' + branch.id);
	const defaultFileWidthRem = persisted<number | undefined>(30, 'defaulFileWidth' + project.id);
	const fileWidthKey = 'fileWidth_';
	let fileWidth: number | undefined = $state(undefined);

	fileWidth = lscache.get(fileWidthKey + branch.id);

	let isLaneCollapsed = $state(projectLaneCollapsed(project.id, branch.id));
	$effect(() => {
		if ($isLaneCollapsed) {
			fileIdSelection.clear();
		}
	});
</script>

<div class="wrapper">
	<Stack {commitBoxOpen} {isLaneCollapsed} />

	{#if selected}
		<div
			class="file-preview"
			bind:this={rsViewport}
			in:slide={{ duration: 180, easing: quintOut, axis: 'x' }}
			style:width={`${fileWidth || $defaultFileWidthRem}rem`}
		>
			<FileCard
				isUnapplied={false}
				conflicted={selected.conflicted}
				file={selected}
				readonly={selected instanceof RemoteFile}
				selectable={$commitBoxOpen && commitId === undefined}
				{commitId}
				onClose={() => {
					fileIdSelection.clear();
				}}
			/>
			<Resizer
				viewport={rsViewport}
				direction="right"
				minWidth={400}
				defaultLineColor="var(--clr-border-2)"
				onWidth={(value) => {
					fileWidth = value / (16 * $userSettings.zoom);
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
		background-color: var(--clr-bg-2);
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

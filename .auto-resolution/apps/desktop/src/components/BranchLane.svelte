<script lang="ts">
	import FileCard from '$components/FileCard.svelte';
	import Resizer from '$components/Resizer.svelte';
	import Stack from '$components/Stack.svelte';
	import { BranchStack } from '$lib/branches/branch';
	import { SelectedOwnership } from '$lib/branches/ownership';
	import { projectLaneCollapsed } from '$lib/config/config';
	import { RemoteFile } from '$lib/files/file';
	import { Project } from '$lib/project/project';
	import { FileIdSelection } from '$lib/selection/fileIdSelection';
	import { getContext, createContextStore } from '@gitbutler/shared/context';
	import { persisted, persistWithExpiration } from '@gitbutler/shared/persisted';
	import { setContext } from 'svelte';
	import { quintOut } from 'svelte/easing';
	import { writable } from 'svelte/store';
	import { slide } from 'svelte/transition';

	const { projectId, stack }: { projectId: string; stack: BranchStack } = $props();

	// BRANCH
	const branchStore = createContextStore(BranchStack, stack);
	const selectedOwnershipStore = createContextStore(
		SelectedOwnership,
		SelectedOwnership.fromBranch(stack)
	);
	const uncommittedFiles = writable(stack.files);

	$effect(() => {
		branchStore.set(stack);
		selectedOwnershipStore.update((o) => o?.update(stack));
		uncommittedFiles.set(stack.files);
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

	let rsViewport: HTMLElement | undefined = $state();

	const commitBoxOpen = persisted<boolean>(false, 'commitBoxExpanded_' + stack.id);
	let width = persistWithExpiration(25, 'fileWidth_' + stack.id, 7 * 1440);

	let isLaneCollapsed = $state(projectLaneCollapsed(project.id, stack.id));
	$effect(() => {
		if ($isLaneCollapsed) {
			fileIdSelection.clear();
		}
	});
</script>

<div class="wrapper">
	<Stack {projectId} {commitBoxOpen} {isLaneCollapsed} />

	{#if selected}
		<div
			class="file-preview"
			bind:this={rsViewport}
			in:slide={{ duration: 180, easing: quintOut, axis: 'x' }}
			style:width={$width + 'rem'}
		>
			<FileCard
				isUnapplied={false}
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
				minWidth={20}
				maxWidth={100}
				onWidth={(value) => ($width = value)}
				imitateBorder
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

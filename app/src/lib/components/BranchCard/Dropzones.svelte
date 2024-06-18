<script lang="ts">
	import { DraggableCommit, DraggableFile, DraggableHunk } from '$lib/dragging/draggables';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { filesToOwnership } from '$lib/vbranches/ownership';
	import { Branch } from '$lib/vbranches/types';
	import Dropzone from '$lib/components/Dropzone/Dropzone.svelte';
	import type { Snippet } from 'svelte';
	import CardOverlay from '$lib/components/Dropzone/CardOverlay.svelte';

	const branchController = getContext(BranchController);
	const branch = getContextStore(Branch);

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	function acceptMoveCommit(data: any) {
		return data instanceof DraggableCommit && data.branchId !== $branch.id && data.isHeadCommit;
	}
	function onMoveCommit(data: DraggableCommit) {
		branchController.moveCommit($branch.id, data.commit.id);
	}

	function acceptBranchDrop(data: any) {
		if (data instanceof DraggableHunk && data.branchId !== $branch.id) {
			return !data.hunk.locked;
		} else if (data instanceof DraggableFile && data.branchId && data.branchId !== $branch.id) {
			return !data.files.some((f) => f.locked);
		} else {
			return false;
		}
	}

	function onBranchDrop(data: DraggableHunk | DraggableFile) {
		if (data instanceof DraggableHunk) {
			const newOwnership = `${data.hunk.filePath}:${data.hunk.id}`;
			branchController.updateBranchOwnership(
				$branch.id,
				(newOwnership + '\n' + $branch.ownership).trim()
			);
		} else if (data instanceof DraggableFile) {
			const newOwnership = filesToOwnership(data.files);
			branchController.updateBranchOwnership(
				$branch.id,
				(newOwnership + '\n' + $branch.ownership).trim()
			);
		}
	}
</script>

<div class="commit-list-item">
	<div class="commit-card-wrapper">
		{@render moveCommitDropzone()}
	</div>
</div>

<!-- We require the dropzones to be nested -->
{#snippet moveCommitDropzone()}
	<Dropzone accepts={acceptMoveCommit} ondrop={onMoveCommit}>
		{@render branchDropDropzone()}

		{#snippet overlay({ hovered, activated })}
			<CardOverlay {hovered} {activated} label="Move here" />
		{/snippet}
	</Dropzone>
{/snippet}

{#snippet branchDropDropzone()}
	<Dropzone accepts={acceptBranchDrop} ondrop={onBranchDrop}>
		{@render children()}

		{#snippet overlay({ hovered, activated })}
			<CardOverlay {hovered} {activated} label="Move here" />
		{/snippet}
	</Dropzone>
{/snippet}

<style>
	.commit-list-item {
		display: flex;
		position: relative;
		padding: 0;
		gap: 8px;
		flex-grow: 1;
		overflow: hidden;
		&:last-child {
			padding-bottom: 0;
		}
	}
	.commit-card-wrapper {
		position: relative;
		width: 100%;
	}
</style>

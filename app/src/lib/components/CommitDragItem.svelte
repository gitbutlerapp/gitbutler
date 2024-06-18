<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import { DraggableCommit, DraggableFile, DraggableHunk } from '$lib/dragging/draggables';
	import { getContext, maybeGetContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { filesToOwnership, filesToSimpleOwnership } from '$lib/vbranches/ownership';
	import { RemoteCommit, Branch, Commit, LocalFile, RemoteFile } from '$lib/vbranches/types';
	import Dropzone from '$lib/components/NewNewDropzone/Dropzone.svelte';
	import type { Snippet } from 'svelte';
	import CardOverlay from '$lib/components/NewNewDropzone/CardOverlay.svelte';

	interface Props {
		commit: Commit | RemoteCommit;
		children: Snippet;
	}

	const { commit, children }: Props = $props();

	const branchController = getContext(BranchController);
	const project = getContext(Project);
	const branch = maybeGetContextStore(Branch);

	function acceptAmend(data: any) {
		if (!$branch) return false;

		if (commit instanceof RemoteCommit) {
			return false;
		}

		if (!project.ok_with_force_push && commit.isRemote) {
			return false;
		}

		if (commit.isIntegrated) {
			return false;
		}

		if (data instanceof DraggableHunk && data.branchId === $branch.id) {
			return true;
		} else if (data instanceof DraggableFile && data.branchId === $branch.id) {
			return true;
		} else {
			return false;
		}
	}

	function onAmend(data: any) {
		if (!$branch) return;

		if (data instanceof DraggableHunk) {
			const newOwnership = `${data.hunk.filePath}:${data.hunk.id}`;
			branchController.amendBranch($branch.id, commit.id, newOwnership);
		} else if (data instanceof DraggableFile) {
			if (data.file instanceof LocalFile) {
				// this is an uncommitted file change being amended to a previous commit
				const newOwnership = filesToOwnership(data.files);
				branchController.amendBranch($branch.id, commit.id, newOwnership);
			} else if (data.file instanceof RemoteFile) {
				// this is a file from a commit, rather than an uncommitted file
				const newOwnership = filesToSimpleOwnership(data.files);
				if (data.commit) {
					branchController.moveCommitFile($branch.id, data.commit.id, commit.id, newOwnership);
				}
			}
		}
	}

	function acceptSquash(data: any) {
		if (!$branch) return false;

		if (commit instanceof RemoteCommit) {
			return false;
		}
		if (!(data instanceof DraggableCommit)) return false;
		if (data.branchId !== $branch.id) return false;

		if (data.commit.isParentOf(commit)) {
			if (data.commit.isIntegrated) return false;
			if (data.commit.isRemote && !project.ok_with_force_push) return false;
			return true;
		} else if (commit.isParentOf(data.commit)) {
			if (commit.isIntegrated) return false;
			if (commit.isRemote && !project.ok_with_force_push) return false;
			return true;
		} else {
			return false;
		}
	}

	function onSquash(data: any) {
		if (!$branch) return;

		if (commit instanceof RemoteCommit) {
			return;
		}
		if (data.commit.isParentOf(commit)) {
			branchController.squashBranchCommit(data.branchId, commit.id);
		} else if (commit.isParentOf(data.commit)) {
			branchController.squashBranchCommit(data.branchId, data.commit.id);
		}
	}
</script>

<div class="commit-list-item">
	<div class="commit-card-wrapper">
		{@render ammendDropzone()}
	</div>
</div>

<!-- We require the dropzones to be nested -->
{#snippet ammendDropzone()}
	<Dropzone accepts={acceptAmend} ondrop={onAmend}>
		{@render squashDropzone()}

		{#snippet overlay({ hovered, activated })}
			<CardOverlay {hovered} {activated} label="Ammend commit" />
		{/snippet}
	</Dropzone>
{/snippet}

{#snippet squashDropzone()}
	<Dropzone accepts={acceptSquash} ondrop={onSquash}>
		{@render children()}

		{#snippet overlay({ hovered, activated })}
			<CardOverlay {hovered} {activated} label="Squash commit" />
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

<script lang="ts">
	import CommitCard from './CommitCard.svelte';
	import DropzoneOverlay from './DropzoneOverlay.svelte';
	import { Project } from '$lib/backend/projects';
	import { DraggableCommit, DraggableFile, DraggableHunk } from '$lib/dragging/draggables';
	import { dropzone } from '$lib/dragging/dropzone';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { filesToOwnership, filesToSimpleOwnership } from '$lib/vbranches/ownership';
	import {
		RemoteCommit,
		Branch,
		type Commit,
		BaseBranch,
		LocalFile,
		RemoteFile
	} from '$lib/vbranches/types';

	export let commit: Commit | RemoteCommit;
	export let isHeadCommit: boolean;
	export let isChained: boolean;
	export let isUnapplied = false;

	const branchController = getContext(BranchController);
	const baseBranch = getContextStore(BaseBranch);
	const project = getContext(Project);
	const branch = getContextStore(Branch);

	function acceptAmend(commit: Commit | RemoteCommit) {
		if (commit instanceof RemoteCommit) {
			return () => false;
		}
		return (data: any) => {
			if (!project.ok_with_force_push && commit.isRemote) {
				return false;
			}

			if (commit.isIntegrated) {
				return false;
			}

			if (data instanceof DraggableHunk && data.branchId == $branch.id) {
				return true;
			} else if (data instanceof DraggableFile && data.branchId == $branch.id) {
				return true;
			} else {
				return false;
			}
		};
	}

	function onAmend(commit: Commit | RemoteCommit) {
		return (data: any) => {
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
		};
	}

	function acceptSquash(commit: Commit | RemoteCommit) {
		if (commit instanceof RemoteCommit) {
			return () => false;
		}
		return (data: any) => {
			if (!(data instanceof DraggableCommit)) return false;
			if (data.branchId != $branch.id) return false;

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
		};
	}

	function onSquash(commit: Commit | RemoteCommit) {
		if (commit instanceof RemoteCommit) {
			return () => false;
		}
		return (data: DraggableCommit) => {
			if (data.commit.isParentOf(commit)) {
				branchController.squashBranchCommit(data.branchId, commit.id);
			} else if (commit.isParentOf(data.commit)) {
				branchController.squashBranchCommit(data.branchId, data.commit.id);
			}
		};
	}
</script>

<div class="commit-list-item">
	{#if isChained}
		<div class="line" />
	{/if}
	<div class="connector" />
	<div
		class="commit-card-wrapper"
		use:dropzone={{
			active: 'amend-dz-active',
			hover: 'amend-dz-hover',
			accepts: acceptAmend(commit),
			onDrop: onAmend(commit)
		}}
		use:dropzone={{
			active: 'squash-dz-active',
			hover: 'squash-dz-hover',
			accepts: acceptSquash(commit),
			onDrop: onSquash(commit)
		}}
	>
		<!-- DROPZONES -->
		<DropzoneOverlay class="amend-dz-marker" label="Amend" />
		<DropzoneOverlay class="squash-dz-marker" label="Squash" />

		<CommitCard
			branch={$branch}
			{commit}
			commitUrl={$baseBranch?.commitUrl(commit.id)}
			{isHeadCommit}
			{isUnapplied}
		/>
	</div>
</div>

<style>
	.commit-list-item {
		display: flex;
		padding: 0 0 var(--size-6) var(--size-16);
		position: relative;

		&:last-child {
			padding-bottom: 0;
		}
	}
	.line {
		position: absolute;
		top: 0;
		left: 0;
		height: 100%;
		width: 1px;
		background-color: var(--clr-border-2);
	}
	.connector {
		width: 1rem;
		height: 1.125rem;
		position: absolute;
		top: 0;
		left: 0;
		border-bottom: 1px solid var(--clr-border-2);
		border-left: 1px solid var(--clr-border-2);
		border-radius: 0 0 0 0.5rem;
	}

	.commit-card-wrapper {
		position: relative;
		width: 100%;
	}
</style>

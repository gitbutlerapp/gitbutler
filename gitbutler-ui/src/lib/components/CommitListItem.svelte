<script lang="ts">
	import CommitCard from './CommitCard.svelte';
	import DropzoneOverlay from './DropzoneOverlay.svelte';
	import { Project } from '$lib/backend/projects';
	import {
		isDraggableHunk,
		type DraggableCommit,
		type DraggableFile,
		type DraggableHunk,
		isDraggableFile,
		isDraggableCommit
	} from '$lib/dragging/draggables';
	import { dropzone } from '$lib/dragging/dropzone';
	import { getContextByClass, getContextStoreByClass } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { filesToOwnership } from '$lib/vbranches/ownership';
	import {
		RemoteCommit,
		type Branch,
		type Commit,
		type AnyFile,
		BaseBranch
	} from '$lib/vbranches/types';
	import { get, type Writable } from 'svelte/store';

	export let branch: Branch;
	export let commit: Commit | RemoteCommit;
	export let isHeadCommit: boolean;
	export let isChained: boolean;
	export let isUnapplied = false;
	export let selectedFiles: Writable<AnyFile[]>;

	const branchController = getContextByClass(BranchController);
	const baseBranch = getContextStoreByClass(BaseBranch);
	const project = getContextByClass(Project);

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

			// only allow to amend the head commit
			if (commit.id != branch.commits.at(0)?.id) {
				return false;
			}

			if (isDraggableHunk(data) && data.branchId == branch.id) {
				return true;
			} else if (isDraggableFile(data) && data.branchId == branch.id) {
				return true;
			} else {
				return false;
			}
		};
	}

	function onAmend(data: DraggableFile | DraggableHunk) {
		if (isDraggableHunk(data)) {
			const newOwnership = `${data.hunk.filePath}:${data.hunk.id}`;
			branchController.amendBranch(branch.id, newOwnership);
		} else if (isDraggableFile(data)) {
			const newOwnership = filesToOwnership([data.current, ...get(data.files)]);
			branchController.amendBranch(branch.id, newOwnership);
		}
	}

	function acceptSquash(commit: Commit | RemoteCommit) {
		if (commit instanceof RemoteCommit) {
			return () => false;
		}
		return (data: any) => {
			if (!isDraggableCommit(data)) return false;
			if (data.branchId != branch.id) return false;

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

<div class="commit-list-item flex w-full items-center gap-x-2 pb-2 pr-4">
	{#if isChained}
		<div class="line" />
	{/if}
	<div class="connector" />
	<div
		class="relative h-full flex-grow overflow-hidden"
		use:dropzone={{
			active: 'amend-dz-active',
			hover: 'amend-dz-hover',
			accepts: acceptAmend(commit),
			onDrop: onAmend
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
			{branch}
			{commit}
			commitUrl={$baseBranch?.commitUrl(commit.id)}
			{isHeadCommit}
			{isUnapplied}
			{selectedFiles}
			branchId={branch.id}
		/>
	</div>
</div>

<style lang="postcss">
	.commit-list-item {
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
		background-color: var(--clr-theme-container-outline-light);
	}
	.connector {
		width: 1rem;
		height: 1.125rem;
		position: absolute;
		top: 0;
		left: 0;
		border-bottom: 1px solid var(--clr-theme-container-outline-light);
		border-left: 1px solid var(--clr-theme-container-outline-light);
		border-radius: 0 0 0 0.5rem;
	}
</style>

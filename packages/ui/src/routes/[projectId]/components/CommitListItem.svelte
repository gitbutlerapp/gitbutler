<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import {
		isDraggableHunk,
		type DraggableCommit,
		type DraggableFile,
		type DraggableHunk,
		isDraggableFile,
		isDraggableCommit
	} from '$lib/draggables';
	import { dropzone } from '$lib/utils/draggable';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch, Commit } from '$lib/vbranches/types';
	import CommitCard from './CommitCard.svelte';

	export let branch: Branch;
	export let project: Project;
	export let commit: Commit;
	export let base: BaseBranch | undefined | null;
	export let isHeadCommit: boolean;
	export let isChained: boolean;
	export let readonly = false;
	export let branchController: BranchController;

	function acceptAmend(commit: Commit) {
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
			const newOwnership = `${data.file.path}:${data.file.hunks.map(({ id }) => id).join(',')}`;
			branchController.amendBranch(branch.id, newOwnership);
		}
	}

	function acceptSquash(commit: Commit) {
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

	function onSquash(commit: Commit) {
		return (data: DraggableCommit) => {
			if (data.commit.isParentOf(commit)) {
				branchController.squashBranchCommit(data.branchId, commit.id);
			} else if (commit.isParentOf(data.commit)) {
				branchController.squashBranchCommit(data.branchId, data.commit.id);
			}
		};
	}

	function resetHeadCommit() {
		if (branch.commits.length > 1) {
			branchController.resetBranch(branch.id, branch.commits[1].id);
		} else if (branch.commits.length === 1 && base) {
			branchController.resetBranch(branch.id, base.baseSha);
		}
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
		<div
			class="amend-dz-marker absolute z-10 hidden h-full w-full items-center justify-center rounded bg-blue-100/70 outline-dashed outline-2 -outline-offset-8 outline-light-600 dark:bg-blue-900/60 dark:outline-dark-300"
		>
			<div class="hover-text font-semibold">Amend</div>
		</div>
		<div
			class="squash-dz-marker absolute z-10 hidden h-full w-full items-center justify-center rounded bg-blue-100/70 outline-dashed outline-2 -outline-offset-8 outline-light-600 dark:bg-blue-900/60 dark:outline-dark-300"
		>
			<div class="hover-text font-semibold">Squash</div>
		</div>

		<CommitCard
			{commit}
			projectId={project.id}
			commitUrl={base?.commitUrl(commit.id)}
			{isHeadCommit}
			{resetHeadCommit}
			{readonly}
		/>
	</div>
	<!-- <div class="reset-head">
			<IconButton icon="cross-small" on:click={() => resetHeadCommit()} />
		</div> -->
</div>

<style lang="postcss">
	.commit-list-item {
		padding: 0 0 var(--space-6) var(--space-16);
		position: relative;
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
		width: 16px;
		height: 18px;
		position: absolute;
		top: 0;
		left: 0;
		border-bottom: 1px solid var(--clr-theme-container-outline-light);
		border-left: 1px solid var(--clr-theme-container-outline-light);
		border-radius: 0 0 0 8px;
	}
</style>

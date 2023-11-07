<script lang="ts">
	import { dropzone } from '$lib/utils/draggable';
	import type { Hunk, File, RemoteCommit } from '$lib/vbranches/types';
	import type { BranchController } from '$lib/vbranches/branchController';
	import CommitCard from './CommitCard.svelte';

	export let branchController: BranchController;
	export let branchId: string;
	export let commit: RemoteCommit;
	export let projectId: string;
	export let commitUrl: string | undefined = undefined;

	function acceptBranchDrop(data: { branchId: string; file?: File; hunk?: Hunk }) {
		if (data.branchId !== branchId) return false;
		return !!data.file || !!data.hunk;
	}

	function onDrop(data: { file?: File; hunk?: Hunk }) {
		if (data.hunk) {
			const newOwnership = `${data.hunk.filePath}:${data.hunk.id}`;
			branchController.amendBranch(branchId, newOwnership);
		} else if (data.file) {
			const newOwnership = `${data.file.path}:${data.file.hunks.map(({ id }) => id).join(',')}`;
			branchController.amendBranch(branchId, newOwnership);
		}
	}
</script>

<div
	class="relative h-full w-full"
	use:dropzone={{
		active: 'amend-dz-active',
		hover: 'amend-dz-hover',
		accepts: acceptBranchDrop,
		onDrop: onDrop
	}}
>
	<div
		class="amend-dz-marker absolute z-10 hidden h-full w-full items-center justify-center rounded bg-blue-100/70 outline-dashed outline-2 -outline-offset-8 outline-light-600 dark:bg-blue-900/60 dark:outline-dark-300"
	>
		<div class="hover-text font-semibold">Amend</div>
	</div>

	<CommitCard {commit} {projectId} {commitUrl} />
</div>

<script lang="ts">
	import DropzoneOverlay from '$lib/components/DropzoneOverlay.svelte';
	import { DraggableCommit } from '$lib/dragging/draggables';
	import { dropzone } from '$lib/dragging/dropzone';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { Branch } from '$lib/vbranches/types';
	import type { ReorderDropzoneIndexer } from '$lib/dragging/reorderDropzoneIndexer';

	export let index: number;
	export let indexer: ReorderDropzoneIndexer;

	const branchController = getContext(BranchController);
	const branch = getContextStore(Branch);

	function accepts(data: any) {
		if (!(data instanceof DraggableCommit)) return false;
		if (data.branchId !== $branch.id) return false;
		if (indexer.dropzoneCommitOffset(index, data.commit.id) === 0) return false;

		return true;
	}

	function onDrop(data: any) {
		if (!(data instanceof DraggableCommit)) return;
		if (data.branchId !== $branch.id) return;

		const offset = indexer.dropzoneCommitOffset(index, data.commit.id);
		branchController.reorderCommit($branch.id, data.commit.id, offset);
	}
</script>

<div
	class="dropzone"
	use:dropzone={{ accepts, onDrop, active: 'reorder-dz-active', hover: 'reorder-dz-hover' }}
>
	<DropzoneOverlay class="reorder-dz-marker" label="Reorder" />
</div>

<style lang="postcss">
	:root {
		/* There is something that is increasing the width by 26 pixels and I'm not quite sure what it is */
		--dropzone-width: calc(100% - 26px);
	}

	:global(.reorder-dz-active .reorder-dz-marker) {
		display: flex !important;
		height: 48px;
		width: var(--dropzone-width);
	}

	:global(.reorder-dz-active) {
		height: 48px;
		width: var(--dropzone-width);
	}
</style>

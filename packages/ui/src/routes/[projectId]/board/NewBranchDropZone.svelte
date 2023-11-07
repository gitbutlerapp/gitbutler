<script lang="ts">
	import { dropzone } from '$lib/utils/draggable';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { File, Hunk } from '$lib/vbranches/types';

	export let branchController: BranchController;

	function accepts(data: { hunk?: Hunk; file?: File }) {
		if (data.hunk !== undefined) return true;
		if (data.file !== undefined) return true;
		return false;
	}

	function onDrop(data: { hunk?: Hunk; file?: File }) {
		if (data.hunk) {
			const ownership = `${data.hunk.filePath}:${data.hunk.id}`;
			branchController.createBranch({ ownership });
		} else if (data.file) {
			const ownership = `${data.file.path}:${data.file.hunks.map(({ id }) => id).join(',')}`;
			branchController.createBranch({ ownership });
		}
	}
</script>

<div
	class="group h-full flex-grow p-2 font-semibold"
	use:dropzone={{
		active: 'new-dz-active',
		hover: 'new-dz-hover',
		onDrop,
		accepts
	}}
>
	<div
		id="new-branch-dz"
		class="call-to-action flex h-full w-96 shrink-0 items-start justify-center text-center opacity-0 transition-all duration-100 group-hover:opacity-100"
	>
		<button
			class="text-color-4 hover:text-color-2 p-2"
			on:click={() => branchController.createBranch({})}
		>
			New virtual branch
		</button>
	</div>
	<div
		class="new-dz-marker text-color-3 border-color-3 hidden h-full w-96 shrink-0 items-center justify-center border-2 border-dashed text-center"
	>
		New branch
	</div>
</div>

<style lang="postcss">
	:global(.new-dz-active .call-to-action) {
		@apply hidden;
	}
	:global(.new-dz-active .new-dz-marker) {
		@apply flex;
	}
	/**
	 * We can't sue dark:[className] because of css isolation, and we can't use :hover
	 * on the element since such events don't seem to trigger on drag. This is a hacky
	 * solution and you're welcome to improve it.
	 */
</style>

<script lang="ts">
	import { dzHighlight } from './dropZone';
	import type { BranchController } from '$lib/vbranches/branchController';

	export let branchController: BranchController;
</script>

<div
	class="group h-full flex-grow p-2 font-semibold"
	role="group"
	use:dzHighlight={{ type: 'text/hunk', hover: 'new-dz-hover', active: 'new-dz-active' }}
	on:drop|stopPropagation={(e) => {
		if (!e.dataTransfer) {
			return;
		}
		const ownership = e.dataTransfer.getData('text/hunk');
		branchController.createBranch({ ownership });
	}}
>
	<div
		id="new-branch-dz"
		class="call-to-action inline-flex h-full w-96 shrink-0 items-center justify-center border-2 border-dashed border-light-600 text-center text-light-600 opacity-0 transition-all duration-200 hover:border-light-700 hover:text-light-800 group-hover:opacity-100 dark:border-dark-300 dark:text-dark-200 hover:dark:border-light-400 hover:dark:text-dark-100"
	>
		<button on:click={() => branchController.createBranch({})}> Click to create new branch </button>
	</div>
	<div
		class="new-dz-marker hidden h-full w-96 shrink-0 items-center justify-center border-2 border-dashed border-light-600 text-center text-light-600 transition-all duration-200 hover:border-light-700 hover:text-light-800 dark:border-dark-300 dark:text-dark-200 hover:dark:border-light-400 hover:dark:text-dark-100"
	>
		Drop here to create branch
	</div>
</div>

<style lang="postcss">
	:global(.new-dz-active .call-to-action) {
		@apply hidden;
	}
	:global(.new-dz-active .new-dz-marker) {
		@apply flex;
	}
	:global(.new-dz-hover .new-dz-marker) {
		@apply border-light-700 text-light-800;
	}
	/**
	 * We can't sue dark:[className] because of css isolation, and we can't use :hover
	 * on the element since such events don't seem to trigger on drag. This is a hacky
	 * solution and you're welcome to improve it.
	 */
	:global(.dark .new-dz-hover .new-dz-marker) {
		@apply border-dark-200 text-dark-100;
	}
</style>

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
		class="new-dz-marker text-color-3 hidden h-full w-96 shrink-0 items-center justify-center border-2 border-dashed border-light-500 text-center dark:border-dark-600 dark:hover:border-dark-600"
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
	:global(.new-dz-hover .new-dz-marker) {
		@apply border-light-600;
	}
	:global(.dark .new-dz-hover .new-dz-marker) {
		@apply border-dark-200;
	}
</style>

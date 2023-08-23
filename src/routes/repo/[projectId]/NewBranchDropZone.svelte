<script lang="ts">
	import { dzHighlight } from './dropZone';
	import type { BranchController } from '$lib/vbranches/branchController';

	export let branchController: BranchController;
</script>

<div
	id="new-branch-dz"
	role="group"
	class="flex h-full w-[22.5rem] shrink-0 justify-center pt-6 text-center text-light-800 dark:text-dark-100"
	use:dzHighlight={{ type: 'text/hunk', hover: 'drop-zone-hover', active: 'drop-zone-active' }}
	on:drop|stopPropagation={(e) => {
		if (!e.dataTransfer) {
			return;
		}
		const ownership = e.dataTransfer.getData('text/hunk');
		branchController.createBranch({ ownership });
	}}
>
	<div class="drop-zone-marker hidden h-36 border border-green-450 p-8">
		<div class="flex h-full flex-col items-center self-center p-2">
			<p>Drop here to create new virtual branch</p>
		</div>
	</div>
</div>

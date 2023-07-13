<script lang="ts">
	import { Button } from '$lib/components';
	import { dzHighlight } from './dropZone';
	import type { BranchController } from '$lib/vbranches';

	export let branchController: BranchController;

	function handleNewVirtualBranch() {
		branchController.createBranch({});
	}
</script>

<div
	id="new-branch-dz"
	role="group"
	class="h-42 ml-4 mt-14 flex w-[22.5rem] shrink-0 justify-center text-center text-light-800 dark:text-dark-100"
	use:dzHighlight={{ type: 'text/hunk', hover: 'drop-zone-hover', active: 'drop-zone-active' }}
	on:drop|stopPropagation={(e) => {
		if (!e.dataTransfer) {
			return;
		}
		const ownership = e.dataTransfer.getData('text/hunk');
		branchController.createBranch({ ownership });
	}}
>
	<div class="bg-green-300" />
	<div class="call-to-action flex-grow rounded border border-light-500 p-8 dark:border-dark-500">
		<div class="flex flex-col items-center gap-y-3 self-center p-2">
			<p>Drag changes or click button to create new virtual branch</p>
			<Button color="purple" height="small" on:click={handleNewVirtualBranch}>
				New virtual branch
			</Button>
		</div>
	</div>
	<div class="drop-zone-marker hidden flex-grow rounded-lg border border-green-450 p-8">
		<div class="flex flex-col items-center gap-y-3 self-center p-2">
			<p>Drop here to create new virtual branch</p>
		</div>
	</div>
</div>

<script lang="ts">
	import { Button } from '$lib/components';
	import { dzHighlight } from './dropZone';
	import type { BranchController } from '$lib/vbranches';
	import { getContext } from 'svelte';
	import { BRANCH_CONTROLLER_KEY } from '$lib/vbranches/branchController';

	const branchController = getContext<BranchController>(BRANCH_CONTROLLER_KEY);

	function handleNewVirtualBranch() {
		branchController.createBranch({});
	}
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
	<div class="call-to-action h-36 p-8">
		<div class="flex flex-col items-center gap-y-3 self-center p-2">
			<p>Drag changes or click button to create new virtual branch</p>
			<Button color="purple" height="small" on:click={handleNewVirtualBranch}>
				New virtual branch
			</Button>
		</div>
	</div>
	<div class="drop-zone-marker hidden h-36 border border-green-450 p-8">
		<div class="flex h-full flex-col items-center self-center p-2">
			<p>Drop here to create new virtual branch</p>
		</div>
	</div>
</div>

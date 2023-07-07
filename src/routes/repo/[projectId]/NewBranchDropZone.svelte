<script lang="ts">
	import type { Branch } from '$lib/api/ipc/vbranches';
	import { Button } from '$lib/components';
	import type { VirtualBranchOperations } from './vbranches';

	export let virtualBranches: VirtualBranchOperations;
	let items: Branch[] = [];
	let dropZone: HTMLDivElement;

	function handleNewVirtualBranch() {
		virtualBranches.createBranch({});
	}

	function isChildOf(child: any, parent: HTMLElement): boolean {
		if (parent === child) return false;
		if (!child.parentElement) return false;
		if (child.parentElement == parent) return true;
		return isChildOf(child.parentElement, parent);
	}
</script>

<div
	id="new-branch-dz"
	class="h-42 ml-4 mt-16 flex w-[22.5rem] shrink-0 justify-center text-center text-light-800 dark:text-dark-100"
	bind:this={dropZone}
	on:dragover|stopPropagation={(e) => {
		if (e.dataTransfer?.types.includes('text/hunk')) e.preventDefault();
		dropZone.classList.add('drag-zone-hover');
	}}
	on:dragleave|stopPropagation={(e) => {
		if (!isChildOf(e.target, dropZone)) {
			dropZone.classList.remove('drag-zone-hover');
		}
	}}
	on:drop|stopPropagation={(e) => {
		if (!e.dataTransfer) {
			return;
		}
		dropZone.classList.remove('drag-zone-hover');
		const ownership = e.dataTransfer.getData('text/hunk');
		virtualBranches.createBranch({ ownership });
	}}
>
	<div class="bg-green-300" />
	<div class="call-to-action flex-grow rounded-lg border border-dashed border-light-600 p-8">
		<div class="flex flex-col items-center gap-y-3 self-center p-2">
			<p>Drag changes or click button to create new virtual branch</p>
			<Button color="purple" height="small" on:click={handleNewVirtualBranch}
				>New virtual branch</Button
			>
		</div>
	</div>
	<div class="drag-zone-marker hidden flex-grow rounded-lg border border-green-450 p-8">
		<div class="flex flex-col items-center gap-y-3 self-center p-2">
			<p>Drop here to add to virtual branch</p>
		</div>
	</div>
</div>

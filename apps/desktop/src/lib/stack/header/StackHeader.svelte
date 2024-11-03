<script lang="ts">
	import ControlSection from './ControlSection.svelte';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import { getContextStore, getContext } from '@gitbutler/shared/context';

	const branchController = getContext(BranchController);
	const branchStore = getContextStore(VirtualBranch);
	const branch = $derived($branchStore);
</script>

<div class="stack-header">
	<ControlSection
		isDefault={branch.selectedForChanges}
		onDefaultSet={async () => {
			await branchController.setSelectedForChanges(branch.id);
		}}
	/>
</div>

<style lang="postcss">
	.stack-header {
		display: flex;
		flex-direction: column;
		width: 100%;
	}
</style>

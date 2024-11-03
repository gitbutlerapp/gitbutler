<script lang="ts">
	import StackControlSection from './StackControlSection.svelte';
	import StackMetaSection from './StackMetaSection.svelte';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';

	interface Props {
		branch: VirtualBranch;
	}

	const branchController = getContext(BranchController);
	// const branchStore = getContextStore(VirtualBranch);
	// const branch = $derived($branchStore);

	const { branch }: Props = $props();
	const nonArchivedSeries = $derived(branch.series.filter((s) => !s.archived));
	const seriesNames = $derived(nonArchivedSeries.map((s) => s.name));
</script>

<div class="stack-header">
	<StackControlSection
		isDefault={branch.selectedForChanges}
		onDefaultSet={async () => {
			await branchController.setSelectedForChanges(branch.id);
		}}
	/>
	<StackMetaSection series={seriesNames} />
</div>

<style lang="postcss">
	.stack-header {
		display: flex;
		flex-direction: column;
		width: 100%;
	}
</style>

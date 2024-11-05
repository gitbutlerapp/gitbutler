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

	const { branch }: Props = $props();
	const nonArchivedSeries = $derived(branch.series.filter((s) => !s.archived));
</script>

<div class="stack-header">
	<StackControlSection
		isDefault={branch.selectedForChanges}
		onDefaultSet={async () => {
			await branchController.setSelectedForChanges(branch.id);
		}}
	/>
	<StackMetaSection series={nonArchivedSeries} />
</div>

<style lang="postcss">
	.stack-header {
		display: flex;
		flex-direction: column;
		width: 100%;
	}
</style>

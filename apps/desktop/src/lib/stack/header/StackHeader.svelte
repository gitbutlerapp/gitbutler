<script lang="ts">
	import HeaderControlSection from './HeaderControlSection.svelte';
	import HeaderMetaSection from './HeaderMetaSection.svelte';
	import { BranchController } from '$lib/vbranches/branchController';
	import { BranchStack } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';
	import { isError } from '@gitbutler/ui/utils/typeguards';

	interface Props {
		branch: BranchStack;
		onCollapseButtonClick: () => void;
	}

	const branchController = getContext(BranchController);

	const { onCollapseButtonClick, branch }: Props = $props();

	const nonArchivedSeries = $derived(
		branch.series.filter((s) => {
			if (isError(s)) return s;
			return !s.archived;
		})
	);
</script>

<div class="stack-header">
	<HeaderControlSection
		isDefault={branch.selectedForChanges}
		onDefaultSet={async () => {
			await branchController.setSelectedForChanges(branch.id);
		}}
	/>
	<HeaderMetaSection series={nonArchivedSeries} {onCollapseButtonClick} />
</div>

<style lang="postcss">
	.stack-header {
		z-index: var(--z-floating);
		position: sticky;
		top: 14px;
		display: flex;
		flex-direction: column;
		width: 100%;

		&::after {
			z-index: -1;
			content: '';
			display: block;
			position: absolute;
			top: -20px;
			left: -14px;
			height: 40px;
			width: calc(100% + 20px);
			background-color: var(--clr-bg-2);
		}
	}
</style>

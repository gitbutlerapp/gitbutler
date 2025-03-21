<script lang="ts">
	import HeaderControlSection from '$components/v3/HeaderControlSection.svelte';
	import HeaderMetaSection from '$components/v3/HeaderMetaSection.svelte';
	import { BranchStack } from '$lib/branches/branch';
	import { BranchController } from '$lib/branches/branchController';
	import { getContext } from '@gitbutler/shared/context';
	import { isError } from '@gitbutler/ui/utils/typeguards';

	interface Props {
		stack: BranchStack;
		onCollapseButtonClick: () => void;
	}

	const branchController = getContext(BranchController);

	const { onCollapseButtonClick, stack }: Props = $props();

	const nonArchivedSeries = $derived(
		stack.series.filter((s) => {
			if (isError(s)) return s;
			return !s.archived;
		})
	);
</script>

<div class="stack-header">
	<HeaderControlSection
		isDefault={stack.selectedForChanges}
		{onCollapseButtonClick}
		onDefaultSet={async () => {
			await branchController.setSelectedForChanges(stack.id);
		}}
	/>
	<HeaderMetaSection series={nonArchivedSeries} {onCollapseButtonClick} stackId={stack.id} />
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

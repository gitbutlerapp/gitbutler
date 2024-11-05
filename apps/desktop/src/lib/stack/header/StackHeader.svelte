<script lang="ts">
	import StackControlSection from './StackControlSection.svelte';
	import StackMetaSection from './StackMetaSection.svelte';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';

	interface Props {
		branch: VirtualBranch;
		onCollapseButtonClick: () => void;
	}

	const branchController = getContext(BranchController);

	const { onCollapseButtonClick, branch }: Props = $props();

	let headerEl: HTMLElement | undefined = $state();
	const nonArchivedSeries = $derived(branch.series.filter((s) => !s.archived));
</script>

<div
	bind:this={headerEl}
	class="stack-header"
	onanimationend={() => {
		headerEl?.classList.remove('wiggle-animation');
	}}
>
	<StackControlSection
		isDefault={branch.selectedForChanges}
		onDefaultSet={async () => {
			await branchController.setSelectedForChanges(branch.id);
			headerEl?.classList.add('wiggle-animation');
		}}
	/>
	<StackMetaSection series={nonArchivedSeries} {onCollapseButtonClick} />
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

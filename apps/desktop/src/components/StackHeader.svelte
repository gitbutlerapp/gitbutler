<script lang="ts">
	import HeaderControlSection from './HeaderControlSection.svelte';
	import HeaderMetaSection from './HeaderMetaSection.svelte';
	import { cloudReviewFunctionality } from '$lib/config/uiFeatureFlags';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { BranchController } from '$lib/vbranches/branchController';
	import { BranchStack } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
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

	const stackPublishingService = getContext(StackPublishingService);
	const canPublish = stackPublishingService.canPublish;
	let publishing = $state<'inert' | 'loading' | 'complete'>('inert');

	async function publishStack() {
		publishing = 'loading';
		await stackPublishingService.upsertStack(stack.id);
		publishing = 'complete';
	}
</script>

<div class="stack-header">
	<HeaderControlSection
		isDefault={stack.selectedForChanges}
		{onCollapseButtonClick}
		onDefaultSet={async () => {
			await branchController.setSelectedForChanges(stack.id);
		}}
	/>
	<HeaderMetaSection series={nonArchivedSeries} {onCollapseButtonClick} />
	{#if $cloudReviewFunctionality && $canPublish}
		<Button onclick={publishStack} loading={publishing === 'loading'}>Publish stack</Button>
	{/if}
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

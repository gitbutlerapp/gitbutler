<script lang="ts">
	import ReviewCreation from '$components/ReviewCreation.svelte';
	import ReviewCreationControls from '$components/ReviewCreationControls.svelte';
	import AsyncRender from '$components/v3/AsyncRender.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { getContext } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
		noDrawer?: boolean;
		oncancel?: () => void;
	};

	const { projectId, stackId, branchName, noDrawer, oncancel }: Props = $props();

	const uiState = getContext(UiState);

	let reviewCreation = $state<ReturnType<typeof ReviewCreation>>();

	function close() {
		uiState.stack(stackId).action.set(undefined);
	}

	const stackService = getContext(StackService);

	const forge = getContext(DefaultForgeFactory);

	const branch = $derived(stackService.branchByName(projectId, stackId, branchName));

	const prNumber = $derived(branch.current.data?.prNumber ?? undefined);
	const prService = $derived(forge.current.prService);
	const prResult = $derived(prNumber ? prService?.get(prNumber) : undefined);
	const pr = $derived(prResult?.current.data);

	const stackPublishingService = getContext(StackPublishingService);

	const canPublish = stackPublishingService.canPublish;

	const canPublishBR = $derived(
		!!($canPublish && branch.current.data?.name && !branch.current.data?.reviewId)
	);
	const canPublishPR = $derived(!!(forge.current.authenticated && !pr));

	function getTitleLabel() {
		if (canPublishBR && canPublishPR) {
			return 'Submit for code review';
		} else if (canPublishBR) {
			return 'Create Butler Request';
		} else if (canPublishPR) {
			return 'Create Pull Request';
		}
		return 'Submit for code review';
	}

	const ctaDisabled = $derived(reviewCreation ? !reviewCreation.imports.creationEnabled : false);
</script>

{#snippet editor()}
	<AsyncRender>
		<div class="review-view" data-testid={TestId.ReviewView}>
			<ReviewCreation
				bind:this={reviewCreation}
				{projectId}
				{stackId}
				{branchName}
				onClose={close}
			/>
			<ReviewCreationControls
				isSubmitting={!!reviewCreation?.imports.isLoading}
				{ctaDisabled}
				{canPublishBR}
				{canPublishPR}
				onCancel={() => {
					close();
					oncancel?.();
				}}
				onSubmit={async () => {
					await reviewCreation?.createReview();
				}}
			/>
		</div>
	</AsyncRender>
{/snippet}

{#if noDrawer}
	<div class="submit-review__container">
		{@render editor()}
	</div>
{:else}
	<Drawer {projectId} title={getTitleLabel()} disableScroll minHeight={20}>
		<div class="submit-review__container">
			{@render editor()}
		</div>
	</Drawer>
{/if}

<style lang="postcss">
	.review-view {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.submit-review__container {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		overflow: hidden;
		gap: 14px;
	}
</style>

<script lang="ts">
	import ReviewCreation from '$components/ReviewCreation.svelte';
	import ReviewCreationControls from '$components/ReviewCreationControls.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
	};

	const { projectId, stackId, branchName }: Props = $props();

	const uiState = getContext(UiState);

	let reviewCreation = $state<ReturnType<typeof ReviewCreation>>();

	function close() {
		uiState.project(projectId).drawerPage.current = 'branch';
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

<Drawer {projectId} {stackId} title={getTitleLabel()} disableScroll minHeight={28}>
	<div class="submit-review__container">
		<ReviewCreation bind:this={reviewCreation} {projectId} {stackId} {branchName} onClose={close} />

		<ReviewCreationControls
			isSubmitting={!!reviewCreation?.imports.isLoading}
			{ctaDisabled}
			{canPublishBR}
			{canPublishPR}
			onCancel={close}
			onSubmit={async () => {
				await reviewCreation?.createReview();
			}}
		/>
	</div>
</Drawer>

<style lang="postcss">
	.submit-review__container {
		flex-grow: 1;
		display: flex;
		flex-direction: column;
		gap: 14px;
		overflow: hidden;
	}
</style>

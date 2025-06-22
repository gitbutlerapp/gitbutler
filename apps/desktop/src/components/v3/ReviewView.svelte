<script lang="ts">
	import ReviewCreation from '$components/ReviewCreation.svelte';
	import ReviewCreationControls from '$components/ReviewCreationControls.svelte';
	import AsyncRender from '$components/v3/AsyncRender.svelte';
	import FloatingCommitBox from '$components/v3/FloatingCommitBox.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { getContext } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
		oncancel?: () => void;
	};

	const { projectId, stackId, branchName, oncancel }: Props = $props();

	const uiState = getContext(UiState);
	const useFloatingPrBox = $derived(uiState.global.useFloatingPrBox);

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

	const canPublishPR = $derived(!!(forge.current.authenticated && !pr));

	const submitDisabled = $derived(reviewCreation ? !reviewCreation.imports.creationEnabled : false);
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
				{submitDisabled}
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

{#if useFloatingPrBox.current}
	<FloatingCommitBox
		onExitFloatingModeClick={() => {
			uiState.global.useFloatingPrBox.set(false);
		}}
	>
		{#snippet header()}
			Create Review
		{/snippet}
		{@render editor()}
	</FloatingCommitBox>
{:else}
	{@render editor()}
{/if}

<style lang="postcss">
	.review-view {
		display: flex;
		flex-direction: column;
		height: 100%;
		gap: 10px;
	}
</style>

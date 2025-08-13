<script lang="ts">
	import FloatingCommitBox from '$components/FloatingCommitBox.svelte';
	import ReviewCreation from '$components/ReviewCreation.svelte';
	import ReviewCreationControls from '$components/ReviewCreationControls.svelte';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { TestId } from '@gitbutler/ui';

	type Props = {
		projectId: string;
		stackId?: string;
		branchName: string;
		oncancel?: () => void;
	};

	const { projectId, stackId, branchName, oncancel }: Props = $props();

	const uiState = inject(UI_STATE);
	const useFloatingBox = uiState.global.useFloatingBox;

	let reviewCreation = $state<ReturnType<typeof ReviewCreation>>();

	function close() {
		uiState.project(projectId).exclusiveAction.set(undefined);
	}

	const stackService = inject(STACK_SERVICE);

	const forge = inject(DEFAULT_FORGE_FACTORY);

	const branch = $derived(stackService.branchByName(projectId, stackId, branchName));

	const prNumber = $derived(branch.current.data?.prNumber ?? undefined);
	const prService = $derived(forge.current.prService);
	const reviewUnit = $derived(prService?.unit.abbr ?? 'PR');
	const prResult = $derived(prNumber ? prService?.get(prNumber) : undefined);
	const pr = $derived(prResult?.current.data);

	const canPublishPR = $derived(!!(forge.current.authenticated && !pr));
</script>

{#snippet editor()}
	<div class="create-review-box" data-testid={TestId.CreateReviewBox}>
		<ReviewCreation bind:this={reviewCreation} {projectId} {stackId} {branchName} onClose={close} />
		<ReviewCreationControls
			isSubmitting={!!reviewCreation?.imports.isLoading}
			{canPublishPR}
			{reviewUnit}
			onCancel={() => {
				close();
				oncancel?.();
			}}
			onSubmit={async () => {
				await reviewCreation?.createReview();
			}}
		/>
	</div>
{/snippet}

{#if useFloatingBox.current}
	<FloatingCommitBox
		onExitFloatingModeClick={() => {
			uiState.global.useFloatingBox.set(false);
		}}
		title={pr ? `Edit ${reviewUnit} #${pr.number}` : `Create ${reviewUnit}`}
	>
		{@render editor()}
	</FloatingCommitBox>
{:else}
	{@render editor()}
{/if}

<style lang="postcss">
	.create-review-box {
		display: flex;
		flex-direction: column;
		height: 100%;
		gap: 10px;
	}
</style>

<script lang="ts">
	import FloatingCommitBox from "$components/commit/FloatingCommitBox.svelte";
	import ReviewCreation from "$components/forge/ReviewCreation.svelte";
	import ReviewCreationControls from "$components/forge/ReviewCreationControls.svelte";
	import { useForgeAuth } from "$lib/forge/forgeAuth.svelte";
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { PR_SERVICE } from "$lib/forge/prService.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
	import { TestId } from "@gitbutler/ui";
	import type { Segment } from "@gitbutler/but-sdk";

	type Props = {
		projectId: string;
		stackId?: string;
		branchName: string;
		segment: Segment;
		branchIndex: number;
		parent: Segment | undefined;
		withForce: boolean;
		stackPrNumbers: (number | undefined)[];
		prNumber?: number;
		oncancel?: () => void;
	};

	const {
		projectId,
		stackId,
		branchName,
		segment,
		branchIndex,
		parent,
		withForce,
		stackPrNumbers,
		prNumber,
		oncancel,
	}: Props = $props();

	const uiState = inject(UI_STATE);
	const useFloatingBox = uiState.global.useFloatingBox;

	let reviewCreation = $state<ReturnType<typeof ReviewCreation>>();

	function close() {
		uiState.project(projectId).exclusiveAction.set(undefined);
	}

	const prService = inject(PR_SERVICE);
	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const reviewUnit = $derived(forgeInfoQuery.response?.unit.abbr ?? "PR");
	const prQuery = $derived(prNumber ? prService.get(projectId, prNumber) : undefined);
	const pr = $derived(prQuery?.response);
	const auth = useForgeAuth(reactive(() => projectId));
	const canPublishPR = $derived(auth.authenticated.current && !pr);
</script>

{#snippet editor()}
	<div class="create-review-box" data-testid={TestId.CreateReviewBox}>
		<ReviewCreation
			bind:this={reviewCreation}
			{projectId}
			{stackId}
			{branchName}
			{segment}
			{branchIndex}
			{parent}
			{withForce}
			{stackPrNumbers}
			onClose={close}
		/>
		<ReviewCreationControls
			isCreatingPR={!!reviewCreation?.imports.isLoading}
			isFormBusy={!!reviewCreation?.imports.isExecuting}
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

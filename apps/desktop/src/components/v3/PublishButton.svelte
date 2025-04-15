<script lang="ts">
	import CanPublishReviewPlugin from '$components/v3/CanPublishReviewPlugin.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { BranchDetails } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		stackId: string;
		branches: BranchDetails[];
	};

	const { projectId, stackId, branches }: Props = $props();
	const uiState = getContext(UiState);

	let canPublishReviewPlugin = $state<ReturnType<typeof CanPublishReviewPlugin>>();

	const lastBranch = $derived(branches.at(-1));
	const branchName = $derived(lastBranch?.name);

	const canPublishBR = $derived(!!canPublishReviewPlugin?.imports.canPublishBR);
	const canPublishPR = $derived(!!canPublishReviewPlugin?.imports.canPublishPR);
	const ctaLabel = $derived(canPublishReviewPlugin?.imports.ctaLabel);

	const hasConflicts = $derived(lastBranch ? lastBranch.isConflicted : false);

	const canPublish = $derived(canPublishBR || canPublishPR);

	function publish() {
		uiState.project(projectId).drawerPage.current = 'review';
	}
</script>

{#if branchName}
	<CanPublishReviewPlugin {projectId} {stackId} {branchName} bind:this={canPublishReviewPlugin} />

	{#if canPublish}
		<div class="publish-button">
			<Button
				style="neutral"
				wide
				disabled={hasConflicts}
				tooltip={hasConflicts
					? 'In order to push, please resolve any conflicted commits.'
					: undefined}
				onclick={publish}
			>
				{ctaLabel}
			</Button>
		</div>
	{/if}
{/if}

<style>
	.publish-button {
		/* This is just here so that the disabled button is still opaque */
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		flex: 1;
	}
</style>

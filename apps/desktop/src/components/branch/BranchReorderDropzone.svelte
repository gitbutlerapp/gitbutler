<script lang="ts">
	import BranchDividerLine from "$components/branch/BranchDividerLine.svelte";
	import BranchDropIndicator from "$components/branch/BranchDropIndicator.svelte";
	import Dropzone from "$components/shared/Dropzone.svelte";
	import { MoveBranchDzHandler } from "$lib/dragging/dropHandlers/branchDropHandler";

	interface Props {
		projectId: string;
		stackId: string | undefined;
		/** The linked worktree whose lane this is, if not a stack's. */
		worktree?: string;
		branchName: string;
		lineColor: string;
		isCommitting: boolean;
		isFirst?: boolean;
		/** The zone under the lane's last branch, anchored on that branch's last commit. */
		isLast?: boolean;
		belowCommitId?: string;
	}

	const {
		projectId,
		stackId,
		worktree,
		branchName,
		lineColor,
		isCommitting,
		isFirst = false,
		isLast = false,
		belowCommitId,
	}: Props = $props();
</script>

{#if !isCommitting}
	{@const moveBranchHandler = new MoveBranchDzHandler(
		projectId,
		stackId,
		branchName,
		worktree,
		belowCommitId,
	)}
	<Dropzone handlers={[moveBranchHandler]}>
		{#snippet overlay({ hovered, activated })}
			{#if isFirst || isLast}
				{#if activated}
					<div
						data-testid="BranchListInsertionDropzone"
						class="dropzone-target"
						class:top-dropzone={isFirst}
						class:bottom-dropzone={isLast}
					>
						<BranchDropIndicator {hovered} />
					</div>
				{/if}
			{:else}
				<div data-testid="BranchListInsertionDropzone" class="dropzone-target">
					<BranchDividerLine {lineColor} />
					{#if activated}
						<BranchDropIndicator {hovered} />
						<BranchDividerLine {lineColor} />
					{/if}
				</div>
			{/if}
		{/snippet}
	</Dropzone>
{:else if !isFirst && !isLast}
	<BranchDividerLine {lineColor} />
{/if}

<style>
	.top-dropzone {
		display: flex;
		flex-direction: column;
		margin-top: -12px;
		padding: 12px 0;
	}

	.bottom-dropzone {
		display: flex;
		flex-direction: column;
		margin-bottom: -12px;
		padding: 12px 0;
	}
</style>

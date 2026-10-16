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
	}

	const {
		projectId,
		stackId,
		worktree,
		branchName,
		lineColor,
		isCommitting,
		isFirst = false,
	}: Props = $props();
</script>

{#if !isCommitting}
	{@const moveBranchHandler = new MoveBranchDzHandler(projectId, stackId, branchName, worktree)}
	<Dropzone handlers={[moveBranchHandler]}>
		{#snippet overlay({ hovered, activated })}
			{#if isFirst}
				{#if activated}
					<div data-testid="BranchListInsertionDropzone" class="dropzone-target top-dropzone">
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
{:else if !isFirst}
	<BranchDividerLine {lineColor} />
{/if}

<style>
	.top-dropzone {
		display: flex;
		flex-direction: column;
		margin-top: -12px;
		padding: 12px 0;
	}
</style>

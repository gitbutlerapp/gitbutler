<script lang="ts">
	import BranchDividerLine from '$components/BranchDividerLine.svelte';
	import BranchLineOverlay from '$components/BranchLineOverlay.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import { MoveBranchDzHandler } from '$lib/branches/dropHandler';
	import type { ForgePrService } from '$lib/forge/interface/forgePrService';
	import type { StackService } from '$lib/stacks/stackService.svelte';

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
		lineColor: string;
		isCommitting: boolean;
		baseBranchName: string | undefined;
		stackService: StackService;
		prService: ForgePrService | undefined;
		isFirst?: boolean;
	}

	const {
		projectId,
		stackId,
		branchName,
		lineColor,
		isCommitting,
		baseBranchName,
		stackService,
		prService,
		isFirst = false
	}: Props = $props();
</script>

{#if !isCommitting && baseBranchName}
	{@const moveBranchHandler = new MoveBranchDzHandler(
		stackService,
		prService,
		projectId,
		stackId,
		branchName,
		baseBranchName
	)}
	<Dropzone handlers={[moveBranchHandler]}>
		{#snippet overlay({ hovered, activated })}
			{#if isFirst}
				{#if activated}
					<div data-testid="BranchListInsertionDropzone" class="dropzone-target top-dropzone">
						<BranchLineOverlay {hovered} />
					</div>
				{/if}
			{:else}
				<div data-testid="BranchListInsertionDropzone" class="dropzone-target">
					<BranchDividerLine {lineColor} />
					{#if activated}
						<BranchLineOverlay {hovered} />
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

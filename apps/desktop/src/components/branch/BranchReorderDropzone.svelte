<script lang="ts">
	import BranchDividerLine from "$components/branch/BranchDividerLine.svelte";
	import BranchDropIndicator from "$components/branch/BranchDropIndicator.svelte";
	import Dropzone from "$components/shared/Dropzone.svelte";
	import { MoveBranchDzHandler } from "$lib/dragging/dropHandlers/branchDropHandler";
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { inject } from "@gitbutler/core/context";
	import type { PrService } from "$lib/forge/prService.svelte";

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
		lineColor: string;
		isCommitting: boolean;
		baseBranchName: string | undefined;
		prService: PrService | undefined;
		isFirst?: boolean;
	}

	const {
		projectId,
		stackId,
		branchName,
		lineColor,
		isCommitting,
		baseBranchName,
		prService,
		isFirst = false,
	}: Props = $props();

	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const unitSymbol = $derived(forgeInfoService.get(projectId).response?.unit.symbol);
</script>

{#if !isCommitting && baseBranchName}
	{@const moveBranchHandler = new MoveBranchDzHandler(
		prService,
		projectId,
		stackId,
		branchName,
		baseBranchName,
		unitSymbol,
	)}
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

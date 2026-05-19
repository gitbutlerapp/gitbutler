<script lang="ts">
	import UnifiedDiffView from "$components/diff/UnifiedDiffView.svelte";
	import UnitySemanticDiffView from "$components/diff/UnitySemanticDiffView.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import YucpBrandWordmark from "$components/shared/YucpBrandWordmark.svelte";
	import { DIFF_SERVICE } from "$lib/hunks/diffService.svelte";
	import { inject } from "@gitbutler/core/context";
	import type { SelectionId } from "$lib/selection/key";
	import type { TreeChange } from "@gitbutler/but-sdk";

	type Props = {
		projectId: string;
		selectable: boolean;
		change: TreeChange;
		selectionId: SelectionId;
		stackId?: string;
		commitId?: string;
		draggable?: boolean;
		topPadding?: boolean;
	};

	const {
		projectId,
		selectable,
		change,
		selectionId,
		stackId,
		commitId,
		draggable,
		topPadding,
	}: Props = $props();

	const diffService = inject(DIFF_SERVICE);
	let viewMode = $state<"unity" | "raw">("unity");
</script>

{#if viewMode === "unity"}
	<div class="diff-section" class:top-padding={topPadding}>
		<div class="unity-mode-toggle" role="group" aria-label="Semantic diff mode">
			<button type="button" class:active={true}>
				<YucpBrandWordmark decorative height="0.75rem" />
			</button>
			<button type="button" onclick={() => (viewMode = "raw")}>Raw</button>
		</div>
		<UnitySemanticDiffView
			{projectId}
			{stackId}
			{change}
			{selectable}
			{selectionId}
			onShowRaw={() => (viewMode = "raw")}
		/>
	</div>
{:else}
	{@const diffQuery = diffService.getDiff(projectId, change)}
	<ReduxResult {projectId} result={diffQuery.result}>
		{#snippet children(diff)}
			<UnifiedDiffView
				{projectId}
				{stackId}
				{commitId}
				{draggable}
				{change}
				{diff}
				{selectable}
				{selectionId}
				{topPadding}
			/>
		{/snippet}
	</ReduxResult>
{/if}

<style lang="postcss">
	.diff-section {
		display: flex;
		flex-direction: column;
		align-self: stretch;
		max-width: 100%;
		padding: 0 14px 14px 14px;
		overflow-x: hidden;
		gap: 14px;

		&.top-padding {
			padding-top: 14px;
		}
	}

	.unity-mode-toggle {
		display: inline-flex;
		align-self: flex-start;
		padding: 2px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-2);

		button {
			padding: 5px 10px;
			border-radius: var(--radius-s);
			color: var(--text-2);
			font-weight: 600;
			font-size: 12px;

			&.active {
				background-color: var(--bg-0);
				box-shadow: var(--fx-shadow-s);
				color: var(--text-1);
			}
		}
	}
</style>

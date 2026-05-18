<script lang="ts">
	import DraggableFileHeader from "$components/diff/DraggableFileHeader.svelte";
	import FilePreviewPlaceholder from "$components/diff/FilePreviewPlaceholder.svelte";
	import UnityLazyDiffView from "$components/diff/UnityLazyDiffView.svelte";
	import UnifiedDiffView from "$components/diff/UnifiedDiffView.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { isUnityPackagePath, isUnitySceneOrPrefabPath } from "$lib/files/unitySemantic";
	import { isExecutableStatus } from "$lib/hunks/change";
	import { DIFF_SERVICE } from "$lib/hunks/diffService.svelte";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { readKey, type SelectionId } from "$lib/selection/key";
	import { inject } from "@gitbutler/core/context";

	type Props = {
		projectId: string;
		selectionId?: SelectionId;
		draggableFiles?: boolean;
		diffOnly?: boolean;
		onclose?: () => void;
		testId?: string;
		scrollContainer?: HTMLDivElement;
		bottomBorder?: boolean;
	};

	let {
		projectId,
		selectionId,
		draggableFiles: draggable,
		diffOnly,
		onclose,
		testId,
		scrollContainer,
		bottomBorder,
	}: Props = $props();

	const idSelection = inject(FILE_SELECTION_MANAGER);
	const diffService = inject(DIFF_SERVICE);
	const binaryDiff = { type: "Binary" } as const;

	const selection = $derived(selectionId ? idSelection.valuesReactive(selectionId) : undefined);
	const lastAdded = $derived(selectionId ? idSelection.getById(selectionId).lastAdded : undefined);

	const selectedFile = $derived.by(() => {
		if (!selectionId || !selection) return;
		if (selection.current.length === 0) return;
		if (selection.current.length === 1 || !$lastAdded) return selection.current[0];
		return readKey($lastAdded.key);
	});

	const stackId = $derived(
		selectionId && `stackId` in selectionId ? selectionId.stackId : undefined,
	);

	const selectable = $derived(selectionId?.type === "worktree");
</script>

<div class="selection-view" data-testid={testId}>
	{#if selectedFile}
		{@const changeQuery = idSelection.changeByKey(projectId, selectedFile)}
		<ReduxResult {projectId} result={changeQuery.result}>
			{#snippet children(change)}
				{@const isExecutable = isExecutableStatus(change.status)}
				{@const isUnitySemantic = isUnitySceneOrPrefabPath(change.path)}
				{@const isUnityPackage = isUnityPackagePath(change.path)}
				<div
					class="selected-change-item"
					class:bottom-border={bottomBorder}
					data-remove-from-panning
				>
					{#if isUnitySemantic}
						{#if !diffOnly}
							<DraggableFileHeader
								selectionId={selectedFile}
								{projectId}
								{scrollContainer}
								{change}
								{draggable}
								executable={isExecutable}
								onCloseClick={onclose}
							/>
						{/if}
						<UnityLazyDiffView
							{projectId}
							{stackId}
							commitId={selectedFile.type === "commit" ? selectedFile.commitId : undefined}
							{draggable}
							{change}
							{selectable}
							selectionId={selectedFile}
							topPadding={diffOnly}
						/>
					{:else if isUnityPackage}
						{#if !diffOnly}
							<DraggableFileHeader
								selectionId={selectedFile}
								{projectId}
								{scrollContainer}
								{change}
								diff={binaryDiff}
								{draggable}
								executable={isExecutable}
								onCloseClick={onclose}
							/>
						{/if}
						<UnifiedDiffView
							{projectId}
							{stackId}
							commitId={selectedFile.type === "commit" ? selectedFile.commitId : undefined}
							{draggable}
							{change}
							diff={binaryDiff}
							{selectable}
							selectionId={selectedFile}
							topPadding={diffOnly}
						/>
					{:else}
						{@const diffQuery = diffService.getDiff(projectId, change)}
						<ReduxResult {projectId} result={diffQuery.result}>
							{#snippet children(diff, env)}
							{#if !diffOnly}
								<DraggableFileHeader
									selectionId={selectedFile}
									projectId={env.projectId}
									{scrollContainer}
									{change}
									{diff}
									{draggable}
									executable={isExecutable}
									onCloseClick={onclose}
								/>
							{/if}
							<UnifiedDiffView
								projectId={env.projectId}
								{stackId}
								commitId={selectedFile.type === "commit" ? selectedFile.commitId : undefined}
								{draggable}
								{change}
								{diff}
								{selectable}
								selectionId={selectedFile}
								topPadding={diffOnly}
							/>
							{/snippet}
						</ReduxResult>
					{/if}
				</div>
			{/snippet}
		</ReduxResult>
	{:else}
		<FilePreviewPlaceholder />
	{/if}
</div>

<style>
	.selection-view {
		display: flex;
		flex-grow: 1;
		width: 100%;
		height: 100%;
	}
	.selected-change-item {
		width: 100%;
		background-color: var(--bg-1);

		&.bottom-border {
			border-bottom: 1px solid var(--border-2);
		}
	}
</style>

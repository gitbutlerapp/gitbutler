<script lang="ts">
	import FileListItemWrapper from '$components/FileListItemWrapper.svelte';
	import FilePreviewPlaceholder from '$components/FilePreviewPlaceholder.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import UnifiedDiffView from '$components/UnifiedDiffView.svelte';
	import { DIFF_SERVICE } from '$lib/hunks/diffService.svelte';
	import {
		INTELLIGENT_SCROLLING_SERVICE,
		scrollingAttachment
	} from '$lib/intelligentScrolling/service';
	import { ID_SELECTION } from '$lib/selection/idSelection.svelte';
	import { readKey, type SelectionId } from '$lib/selection/key';
	import { inject } from '@gitbutler/shared/context';

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
		bottomBorder
	}: Props = $props();

	const idSelection = inject(ID_SELECTION);
	const diffService = inject(DIFF_SERVICE);
	const intelligentScrollingService = inject(INTELLIGENT_SCROLLING_SERVICE);

	const selection = $derived(selectionId ? idSelection.valuesReactive(selectionId) : undefined);
	const lastAdded = $derived(selectionId ? idSelection.getById(selectionId).lastAdded : undefined);

	const selectedFile = $derived.by(() => {
		if (!selectionId || !selection) return;
		if (selection.current.length === 0) return;
		if (selection.current.length === 1 || !$lastAdded) return selection.current[0];
		return readKey($lastAdded.key);
	});

	const stackId = $derived(
		selectionId && `stackId` in selectionId ? selectionId.stackId : undefined
	);

	const selectable = $derived(selectionId?.type === 'worktree');
</script>

<div
	class="selection-view"
	data-testid={testId}
	{@attach scrollingAttachment(intelligentScrollingService, stackId, 'diff')}
>
	{#if selectedFile}
		{@const changeResult = idSelection.changeByKey(projectId, selectedFile)}
		<ReduxResult {projectId} result={changeResult.current}>
			{#snippet children(change)}
				{@const diffResult = diffService.getDiff(projectId, change)}
				<ReduxResult {projectId} result={diffResult.current}>
					{#snippet children(diff, env)}
						{@const isExecutable = true}
						<div
							class="selected-change-item"
							class:bottom-border={bottomBorder}
							data-remove-from-panning
						>
							{#if !diffOnly}
								<FileListItemWrapper
									selectionId={selectedFile}
									projectId={env.projectId}
									{scrollContainer}
									{change}
									{diff}
									{draggable}
									isHeader
									executable={!!isExecutable}
									listMode="list"
									onCloseClick={onclose}
									focusableOpts={{ list: true }}
								/>
							{/if}
							<UnifiedDiffView
								projectId={env.projectId}
								{stackId}
								commitId={selectedFile.type === 'commit' ? selectedFile.commitId : undefined}
								{draggable}
								{change}
								{diff}
								{selectable}
								selectionId={selectedFile}
								topPadding={diffOnly}
							/>
						</div>
					{/snippet}
				</ReduxResult>
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
		background-color: var(--clr-bg-1);

		&.bottom-border {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}
</style>

<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileListItemWrapper from '$components/v3/FileListItemWrapper.svelte';
	import SelectTopreviewPlaceholder from '$components/v3/SelectTopreviewPlaceholder.svelte';
	import UnifiedDiffView from '$components/v3/UnifiedDiffView.svelte';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import {
		IntelligentScrollingService,
		scrollingAttachment
	} from '$lib/intelligentScrolling/service';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { readKey, type SelectionId } from '$lib/selection/key';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		selectionId?: SelectionId;
		draggableFiles?: boolean;
		diffOnly?: boolean;
		onclose?: () => void;
		testId?: string;
		bottomBorder?: boolean;
	};

	let {
		projectId,
		selectionId,
		draggableFiles: draggable,
		diffOnly,
		onclose,
		testId,
		bottomBorder
	}: Props = $props();

	const [idSelection, diffService, intelligentScrollingService] = inject(
		IdSelection,
		DiffService,
		IntelligentScrollingService
	);

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
</script>

<div
	class="selection-view"
	data-testid={testId}
	{@attach scrollingAttachment(intelligentScrollingService, stackId, 'diff')}
>
	{#if selectedFile}
		{@const changeResult = idSelection.changeByKey(projectId, selectedFile)}
		<ScrollableContainer wide zIndex="var(--z-lifted)">
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
										{change}
										{diff}
										{draggable}
										isHeader
										executable={!!isExecutable}
										listMode="list"
										onCloseClick={() => {
											if (idSelection) {
												idSelection.remove(selectedFile.path, selectedFile);
											}
											onclose?.();
										}}
									/>
								{/if}
								<UnifiedDiffView
									projectId={env.projectId}
									{stackId}
									commitId={selectedFile.type === 'commit' ? selectedFile.commitId : undefined}
									{draggable}
									{change}
									{diff}
									selectable
									selectionId={selectedFile}
									topPadding={diffOnly}
								/>
							</div>
						{/snippet}
					</ReduxResult>
				{/snippet}
			</ReduxResult>
		</ScrollableContainer>
	{:else}
		<SelectTopreviewPlaceholder />
	{/if}
</div>

<style>
	.selection-view {
		display: flex;
		flex-grow: 1;
		width: 100%;
		height: 100%;
		overflow: hidden;
	}
	.selected-change-item {
		background-color: var(--clr-bg-1);

		&.bottom-border {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}
</style>

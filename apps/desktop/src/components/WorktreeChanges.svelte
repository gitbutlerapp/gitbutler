<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import FileList from '$components/FileList.svelte';
	import FileListMode from '$components/FileListMode.svelte';
	import WorktreeChangesSelectAll from '$components/WorktreeChangesSelectAll.svelte';
	import { UncommitDzHandler } from '$lib/commits/dropHandler';
	import { DIFF_SERVICE } from '$lib/hunks/diffService.svelte';
	import { AssignmentDropHandler } from '$lib/hunks/dropHandler';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { createWorktreeSelection } from '$lib/selection/key';
	import { UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';

	import { Badge, TestId } from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import { type Snippet } from 'svelte';
	import type { DropzoneHandler } from '$lib/dragging/handler';

	type Props = {
		projectId: string;
		stackId?: string;
		title: string;
		mode?: 'unassigned' | 'assigned';
		onDropzoneActivated?: (activated: boolean) => void;
		onDropzoneHovered?: (hovered: boolean) => void;
		emptyPlaceholder?: Snippet;
		foldButton?: Snippet;
		onFileClick?: (index: number) => void;
		onscrollexists?: (exists: boolean) => void;
	};

	let {
		projectId,
		stackId,
		title,
		mode = 'unassigned',
		onDropzoneActivated,
		onDropzoneHovered,
		emptyPlaceholder,
		foldButton,
		onFileClick,
		onscrollexists
	}: Props = $props();

	// Create a unique persist ID based on stackId and mode (both are static props)
	const persistId = stackId ? `worktree-${mode}-${stackId}` : `worktree-${mode}`;

	const stackService = inject(STACK_SERVICE);
	const diffService = inject(DIFF_SERVICE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const uiState = inject(UI_STATE);
	const idSelection = inject(FILE_SELECTION_MANAGER);

	// Create selectionId for this worktree lane
	const selectionId = $derived(createWorktreeSelection({ stackId }));

	const uncommitDzHandler = $derived(
		new UncommitDzHandler(projectId, stackService, uiState, stackId)
	);

	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(
		exclusiveAction?.type === 'commit' &&
			(exclusiveAction.stackId === stackId || stackId === undefined)
	);

	const changes = $derived(uncommittedService.changesByStackId(stackId || null));

	let listMode: 'list' | 'tree' = $state('list');

	let scrollTopIsVisible = $state(true);

	const assignmentDZHandler = $derived(
		new AssignmentDropHandler(projectId, diffService, uncommittedService, stackId, idSelection)
	);

	function getDropzoneLabel(handler: DropzoneHandler | undefined): string {
		if (handler instanceof UncommitDzHandler) {
			return 'Uncommit';
		} else if (mode === 'assigned') {
			return 'Assign';
		} else {
			return 'Unassign';
		}
	}
</script>

{#snippet fileList()}
	<FileList
		dataTestId={TestId.UncommittedChanges_FileList}
		draggableFiles
		{selectionId}
		showCheckboxes={isCommitting}
		changes={changes.current}
		{projectId}
		{listMode}
		{stackId}
		{onFileClick}
		showLockedIndicator={mode === 'unassigned'}
	/>
{/snippet}

<Dropzone
	handlers={[uncommitDzHandler, assignmentDZHandler].filter(isDefined)}
	maxHeight
	onActivated={onDropzoneActivated}
	onHovered={onDropzoneHovered}
>
	{#snippet overlay({ hovered, activated, handler })}
		<CardOverlay {hovered} {activated} label={getDropzoneLabel(handler)} />
	{/snippet}

	<div class="uncommitted-changes-wrap">
		{#if mode === 'unassigned' || changes.current.length > 0}
			<div
				role="presentation"
				data-testid={TestId.UncommittedChanges_Header}
				class="worktree-header"
				class:sticked-top={!scrollTopIsVisible}
				use:focusable
			>
				{#if foldButton}
					{@render foldButton()}
				{/if}

				<div class="worktree-header__content">
					{#if isCommitting && changes.current.length > 0}
						<WorktreeChangesSelectAll {stackId} />
					{/if}
					<div class="worktree-header__title truncate">
						<h3 class="text-14 text-semibold truncate">{title}</h3>
						<Badge>{changes.current.length}</Badge>
					</div>
				</div>
				{#if changes.current.length > 0}
					<FileListMode bind:mode={listMode} {persistId} />
				{/if}
			</div>
		{/if}

		{#if changes.current.length > 0}
			<ScrollableContainer
				{onscrollexists}
				onscrollTop={(visible) => {
					scrollTopIsVisible = visible;
				}}
				enableDragScroll={mode === 'assigned'}
			>
				{@render fileList()}
			</ScrollableContainer>
		{:else}
			{@render emptyPlaceholder?.()}
		{/if}
	</div>
</Dropzone>

<style>
	.uncommitted-changes-wrap {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		background-color: var(--clr-bg-1);
	}

	/* HEADER */
	.worktree-header {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		width: 100%;
		height: 42px;
		padding: 0 10px 0 14px;
		gap: 6px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
	}

	.worktree-header__content {
		display: flex;
		flex: 1;
		align-items: center;
		overflow: hidden;
		gap: 10px;
	}

	.worktree-header__title {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	/* MODIFIERS */
	.sticked-top {
		border-bottom-color: var(--clr-border-2);
	}
</style>

<script lang="ts">
	import FileListItems from "$components/files/FileListItems.svelte";
	import FileListProvider from "$components/files/FileListProvider.svelte";
	import FileListViewToggle from "$components/files/FileListViewToggle.svelte";
	import WorktreeChangesSelectAll from "$components/files/WorktreeChangesSelectAll.svelte";
	import ScrollableContainer from "$components/shared/AppScrollableContainer.svelte";
	import Dropzone from "$components/shared/Dropzone.svelte";
	import DropzoneOverlay from "$components/shared/DropzoneOverlay.svelte";
	import { UncommitDzHandler } from "$lib/dragging/dropHandlers/commitDropHandler";
	import { AssignmentDropHandler } from "$lib/dragging/dropHandlers/hunkDropHandler";
	import { DIFF_SERVICE } from "$lib/hunks/diffService.svelte";
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { WORKING_FILES_BROADCAST } from "$lib/irc/workingFilesBroadcast.svelte";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { createWorktreeSelection } from "$lib/selection/key";
	import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject, injectOptional } from "@gitbutler/core/context";

	import { Badge, TestId } from "@gitbutler/ui";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import { isDefined } from "@gitbutler/ui/utils/typeguards";
	import { untrack, type Snippet } from "svelte";
	import type { DropzoneHandler } from "$lib/dragging/handler";

	type Props = {
		projectId: string;
		stackId?: string;
		title: string;
		mode?: "unassigned" | "assigned";
		onDropzoneActivated?: (activated: boolean) => void;
		onDropzoneHovered?: (hovered: boolean) => void;
		emptyPlaceholder?: Snippet;
		foldButton?: Snippet;
		onFileClick?: (index: number) => void;
		onscrollexists?: (exists: boolean) => void;
		visibleRange?: { start: number; end: number };
		maxListHeight?: string;
		scrollAreaEl?: HTMLDivElement;
	};

	let {
		projectId,
		stackId,
		title,
		mode = "unassigned",
		onDropzoneActivated,
		onDropzoneHovered,
		emptyPlaceholder,
		foldButton,
		onFileClick,
		onscrollexists,
		visibleRange,
		maxListHeight,
		scrollAreaEl = $bindable(),
	}: Props = $props();

	// Create a unique persist ID based on stackId and mode (both are static props)
	const persistId = untrack(() => (stackId ? `worktree-${mode}-${stackId}` : `worktree-${mode}`));

	const diffService = inject(DIFF_SERVICE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const uiState = inject(UI_STATE);
	const idSelection = inject(FILE_SELECTION_MANAGER);
	const ircApiService = injectOptional(IRC_API_SERVICE, undefined);
	const workingFilesBroadcast = injectOptional(WORKING_FILES_BROADCAST, undefined);

	const workingFilesChannel = $derived(workingFilesBroadcast?.channel);
	const workingFilesQuery = $derived(
		ircApiService && workingFilesChannel
			? ircApiService.workingFiles({ channel: workingFilesChannel })
			: undefined,
	);
	const ircWorkingFiles = $derived(workingFilesQuery?.response);

	// Create selectionId for this worktree lane
	const selectionId = $derived(createWorktreeSelection({ stackId }));

	const uncommitDzHandler = $derived(new UncommitDzHandler(projectId, stackId));

	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(
		exclusiveAction?.type === "commit" &&
			(exclusiveAction.stackId === stackId || stackId === undefined),
	);

	const changes = $derived(uncommittedService.changesByStackId(stackId || null));

	let listMode: "list" | "tree" = $state("list");

	let scrollTopIsVisible = $state(true);

	const assignmentDZHandler = $derived(
		new AssignmentDropHandler(projectId, diffService, uncommittedService, stackId, idSelection),
	);

	function getDropzoneLabel(handler: DropzoneHandler | undefined): string {
		if (handler instanceof UncommitDzHandler) {
			return "Uncommit";
		} else if (mode === "assigned") {
			return "Assign";
		} else {
			return "Unassign";
		}
	}
</script>

{#snippet fileList()}
	<FileListProvider changes={changes.current} {selectionId}>
		<FileListItems
			{projectId}
			{stackId}
			mode={listMode}
			showCheckboxes={isCommitting}
			draggable
			showLockedIndicator
			{visibleRange}
			{ircWorkingFiles}
			dataTestId={TestId.UncommittedChanges_FileList}
			onselect={onFileClick && ((_change, index) => onFileClick(index))}
		/>
	</FileListProvider>
{/snippet}

<Dropzone
	handlers={[uncommitDzHandler, assignmentDZHandler].filter(isDefined)}
	maxHeight
	onActivated={onDropzoneActivated}
	onHovered={onDropzoneHovered}
>
	{#snippet overlay({ hovered, activated, handler })}
		<DropzoneOverlay
			{hovered}
			{activated}
			label={getDropzoneLabel(handler)}
			visible={mode === "assigned" && changes.current.length === 0 && !activated}
		/>
	{/snippet}

	<div class="uncommitted-changes-wrap">
		{#if mode === "unassigned" || changes.current.length > 0}
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
					<FileListViewToggle bind:mode={listMode} {persistId} />
				{/if}
			</div>
		{/if}

		{#if changes.current.length > 0}
			<ScrollableContainer
				{onscrollexists}
				onscrollTop={(visible) => {
					scrollTopIsVisible = visible;
				}}
				enableDragScroll={mode === "assigned"}
				maxHeight={maxListHeight}
				bind:scrollableEl={scrollAreaEl}
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
		background-color: var(--bg-1);
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
		border-bottom: 1px solid var(--border-2);
		background-color: var(--bg-2);
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
		border-bottom-color: var(--border-2);
	}
</style>

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
	import { pathIsLocallyIgnored, WORKTREE_SERVICE } from "$lib/worktree/worktreeService.svelte";
	import { inject, injectOptional } from "@gitbutler/core/context";

	import { Badge, ContextMenu, ContextMenuItem, ContextMenuSection, TestId } from "@gitbutler/ui";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import { isDefined } from "@gitbutler/ui/utils/typeguards";
	import { untrack, type Snippet } from "svelte";
	import type { DropzoneHandler } from "$lib/dragging/handler";
	import type { TreeChange } from "@gitbutler/but-sdk";

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
	}: Props = $props();

	// Create a unique persist ID based on stackId and mode (both are static props)
	const persistId = untrack(() => (stackId ? `worktree-${mode}-${stackId}` : `worktree-${mode}`));

	const diffService = inject(DIFF_SERVICE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const worktreeService = inject(WORKTREE_SERVICE);
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
	const localIgnoredPathsQuery = $derived(
		mode === "unassigned" ? worktreeService.localIgnoredPaths(projectId) : undefined,
	);
	const localIgnoredPaths = $derived(localIgnoredPathsQuery?.response ?? []);
	const compactLocalIgnoredPaths = $derived.by(() => compactIgnoredPaths(localIgnoredPaths));
	const unfilteredWorktreeDataQuery = $derived(
		mode === "unassigned" ? worktreeService.unfilteredWorktreeData(projectId) : undefined,
	);
	const locallyIgnoredChanges = $derived.by(() => {
		if (mode !== "unassigned" || compactLocalIgnoredPaths.length === 0) return [];
		const hiddenChanges = (unfilteredWorktreeDataQuery?.response?.rawChanges ?? []).filter((change) =>
			pathIsLocallyIgnored(change.path, localIgnoredPaths),
		);
		const hiddenChangePaths = new Set(hiddenChanges.map((change) => change.path));
		const placeholderChanges = compactLocalIgnoredPaths
			.filter((path) => !hiddenChangePaths.has(path))
			.map(createLocalIgnoredPlaceholderChange);

		return [...hiddenChanges, ...placeholderChanges];
	});
	let showLocalIgnored = $state(false);
	const visibleLocalIgnoredChanges = $derived(showLocalIgnored ? locallyIgnoredChanges : []);
	const displayChanges = $derived([...changes.current, ...visibleLocalIgnoredChanges]);

	let listMode: "list" | "tree" = $state("list");
	let localIgnoredContextMenuOpen = $state(false);
	let localIgnoredContextMenuTarget = $state<MouseEvent | HTMLElement>();

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

	function getDropzoneLoadingLabel(handler: DropzoneHandler | undefined): string {
		if (handler instanceof UncommitDzHandler) {
			return `Uncommitting from ${title}`;
		}
		if (mode === "unassigned") {
			return `Moving to ${title}`;
		}
		return `${getDropzoneLabel(handler)}ing to ${title}`;
	}

	function onContainerContextMenu(e: MouseEvent) {
		if (mode !== "unassigned" || localIgnoredPaths.length === 0) return;

		e.preventDefault();
		localIgnoredContextMenuTarget = e;
		localIgnoredContextMenuOpen = true;
	}

	function createLocalIgnoredPlaceholderChange(path: string): TreeChange {
		return {
			path,
			pathBytes: Array.from(new TextEncoder().encode(path)),
			status: {
				type: "Modification",
				subject: {
					previousState: { id: "", kind: "Blob" },
					state: { id: "", kind: "Blob" },
					flags: null,
				},
			},
		};
	}

	function compactIgnoredPaths(paths: string[]): string[] {
		const sortedPaths = [...new Set(paths)].sort((a, b) => a.localeCompare(b));
		const compacted: string[] = [];

		for (const path of sortedPaths) {
			const parentPath = compacted.find((parent) => path === parent || path.startsWith(`${parent}/`));
			if (!parentPath) {
				compacted.push(path);
			}
		}

		return compacted;
	}
</script>

{#snippet fileList()}
	<FileListProvider changes={displayChanges} {selectionId}>
		<FileListItems
			{projectId}
			{stackId}
			mode={listMode}
			showCheckboxes={isCommitting}
			draggable
			showLockedIndicator
			{visibleRange}
			{ircWorkingFiles}
			{localIgnoredPaths}
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
	{#snippet overlay({ hovered, activated, handler, dropping })}
		<DropzoneOverlay
			{hovered}
			{activated}
			label={getDropzoneLabel(handler)}
			loading={dropping}
			loadingLabel={getDropzoneLoadingLabel(handler)}
			visible={mode === "assigned" && changes.current.length === 0 && !activated}
		/>
	{/snippet}

	<div class="uncommitted-changes-wrap" role="presentation" oncontextmenu={onContainerContextMenu}>
		{#if mode === "unassigned" || displayChanges.length > 0}
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
						<Badge>{displayChanges.length}</Badge>
					</div>
				</div>
				{#if displayChanges.length > 0}
					<FileListViewToggle bind:mode={listMode} {persistId} />
				{/if}
			</div>
		{/if}

		{#if displayChanges.length > 0}
			<ScrollableContainer
				{onscrollexists}
				onscrollTop={(visible) => {
					scrollTopIsVisible = visible;
				}}
				enableDragScroll={mode === "assigned"}
			>
				{@render fileList()}
			</ScrollableContainer>
		{:else}
			{@render emptyPlaceholder?.()}
		{/if}
	</div>
</Dropzone>

{#if localIgnoredContextMenuOpen}
	<ContextMenu
		target={localIgnoredContextMenuTarget}
		side="bottom"
		onclose={() => (localIgnoredContextMenuOpen = false)}
	>
		<ContextMenuSection>
			<ContextMenuItem
				label={showLocalIgnored ? "Hide ignored" : "Show ignored"}
				icon={showLocalIgnored ? "eye-closed" : "eye"}
				onclick={() => {
					showLocalIgnored = !showLocalIgnored;
					localIgnoredContextMenuOpen = false;
				}}
			/>
		</ContextMenuSection>
	</ContextMenu>
{/if}

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

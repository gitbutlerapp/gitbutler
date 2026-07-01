<!-- This is a V3 replacement for `FileContextMenu.svelte` -->
<script lang="ts">
	import AbsorbPlanModal from "$components/stack/AbsorbPlanModal.svelte";
	import DiscardChangesModal from "$components/workspace/DiscardChangesModal.svelte";
	import StashIntoBranchModal from "$components/workspace/StashIntoBranchModal.svelte";
	import { BACKEND } from "$lib/backend";
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { getEditorUri, URL_SERVICE } from "$lib/backend/url";
	import { changesToDiffSpec } from "$lib/commits/utils";
	import { FILE_SERVICE } from "$lib/files/fileService";
	import { isTreeChange } from "$lib/hunks/change";
	import { vscodePath } from "$lib/project/project";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { UI_STATE, withStackBusy } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		ContextMenu,
		ContextMenuItem,
		ContextMenuItemSubmenu,
		ContextMenuSection,
		chipToasts,
		TestId,
	} from "@gitbutler/ui";
	import type { SelectionId } from "$lib/selection/key";
	import type { TreeChange } from "@gitbutler/but-sdk";

	type Props = {
		projectId: string;
		stackId: string | undefined;
		selectionId: SelectionId;
		trigger?: HTMLElement;
		leftClickTrigger?: HTMLElement;
		editMode?: boolean;
		align?: "start" | "center" | "end";
		onopen?: () => void;
		onclose?: () => void;
	};

	type ChangedFilesItem = {
		changes: TreeChange[];
	};

	function isChangedFilesItem(item: unknown): item is ChangedFilesItem {
		return (
			typeof item === "object" &&
			item !== null &&
			"changes" in item &&
			Array.isArray(item.changes) &&
			item.changes.every(isTreeChange)
		);
	}

	type ChangedFolderItem = ChangedFilesItem & {
		path: string;
	};

	function isChangedFolderItem(item: ChangedFilesItem): item is ChangedFolderItem {
		return "path" in item && typeof item.path === "string";
	}

	const {
		trigger,
		leftClickTrigger,
		selectionId,
		stackId,
		projectId,
		editMode = false,
		align,
		onopen,
		onclose,
	}: Props = $props();
	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);
	const defaultCodeEditor = uiState.global.defaultCodeEditor;
	const idSelection = inject(FILE_SELECTION_MANAGER);
	const fileService = inject(FILE_SERVICE);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const backend = inject(BACKEND);
	const [, absorbingChanges] = stackService.absorb;
	const projectService = inject(PROJECTS_SERVICE);

	const isUncommitted = $derived(selectionId.type === "worktree");

	// Platform-specific label for "Show in Finder/Explorer/File Manager"
	const showInFolderLabel = (() => {
		switch (backend.platformName) {
			case "macos":
				return "Show in Finder";
			case "windows":
				return "Show in Explorer";
			default:
				return "Show in File Manager";
		}
	})();

	let menuOpen = $state(false);
	let menuTarget = $state<MouseEvent | HTMLElement>();
	let menuItem = $state<ChangedFilesItem>();

	let discardModal: ReturnType<typeof DiscardChangesModal>;
	let stashModal: ReturnType<typeof StashIntoBranchModal>;
	let absorbModal: ReturnType<typeof AbsorbPlanModal>;

	function isDeleted(item: ChangedFilesItem): boolean {
		if (isChangedFolderItem(item)) {
			return false;
		}
		return item.changes.some((change) => {
			return change.status.type === "Deletion";
		});
	}

	function getItemPath(item: ChangedFilesItem): string | null {
		if (isChangedFolderItem(item)) {
			return item.path;
		}
		if (item.changes.length === 1) {
			return item.changes[0]!.path;
		}
		return null;
	}

	export function open(e: MouseEvent | HTMLElement, newItem: ChangedFilesItem) {
		menuTarget = e;
		menuItem = newItem;
		menuOpen = true;
	}

	export function close() {
		menuOpen = false;
	}

	async function uncommitChanges(stackId: string, commitId: string, changes: TreeChange[]) {
		menuOpen = false;
		await withStackBusy(uiState, projectId, { commitId, stackIds: [stackId] }, async () => {
			await stackService.uncommitChanges({
				projectId,
				stackId,
				commitId,
				changes: changesToDiffSpec(changes),
				dryRun: false,
			});
			const selectedFiles = changes.map((change) => ({ ...selectionId, path: change.path }));

			// Unselect the uncommitted files
			idSelection.removeMany(selectedFiles);
		});
	}
</script>

{#if menuOpen && menuItem}
	{@const item = menuItem}
	{@const deletion = isDeleted(item)}
	{@const itemPath = getItemPath(item)}
	<ContextMenu
		{leftClickTrigger}
		rightClickTrigger={trigger}
		side="bottom"
		{align}
		target={menuTarget}
		{onopen}
		onclose={() => {
			menuOpen = false;
			onclose?.();
		}}
	>
		{#if isChangedFilesItem(item)}
			{#if item.changes.length > 0 && !editMode}
				<ContextMenuSection>
					{@const changes = item.changes}
					{#if isUncommitted}
						<ContextMenuItem
							label="Discard changes…"
							testId={TestId.FileListItemContextMenu_DiscardChanges}
							icon="bin"
							onclick={() => {
								discardModal.show(item);
								menuOpen = false;
							}}
						/>
						<ContextMenuItem
							label="Stash into branch…"
							icon="branch-bottom-up-arrow"
							onclick={() => {
								stashModal.show(item);
								menuOpen = false;
							}}
						/>
						<ContextMenuItem
							label="Absorb changes"
							icon="commit-absorb"
							testId={TestId.FileListItemContextMenu_Absorb}
							onclick={() => {
								absorbModal.show(item.changes);
								menuOpen = false;
							}}
							disabled={absorbingChanges.current.isLoading}
						/>
					{/if}
					{#if selectionId.type === "commit" && stackId && !editMode}
						{@const commitId = selectionId.commitId}
						<ContextMenuItem
							label="Uncommit changes"
							icon="commit-undo"
							onclick={async () => uncommitChanges(stackId, commitId, changes)}
						/>
					{/if}
				</ContextMenuSection>
			{/if}

			{#if itemPath}
				<ContextMenuSection>
					<ContextMenuItemSubmenu label="Copy path" icon="copy">
						{#snippet submenu(_sub)}
							<ContextMenuSection>
								<ContextMenuItem
									label="Copy path"
									onclick={async () => {
										menuOpen = false;
										const project = await projectService.fetchProject(projectId);
										const projectPath = project?.path;
										if (projectPath) {
											const absPath = await backend.joinPath(projectPath, itemPath);

											await clipboardService.write(absPath, {
												message: "Absolute path copied",
												errorMessage: "Failed to copy absolute path",
											});
										}
									}}
								/>
								<ContextMenuItem
									label="Copy relative path"
									onclick={async () => {
										menuOpen = false;
										await clipboardService.write(itemPath, {
											message: "Relative path copied",
											errorMessage: "Failed to copy relative path",
										});
									}}
								/>
							</ContextMenuSection>
						{/snippet}
					</ContextMenuItemSubmenu>
				</ContextMenuSection>
			{/if}

			<ContextMenuSection>
				{#if !isChangedFolderItem(item)}
					<ContextMenuItem
						label="Open in {defaultCodeEditor.current.displayName}"
						icon="open-in-ide"
						disabled={deletion}
						onclick={async () => {
							menuOpen = false;
							try {
								const project = await projectService.fetchProject(projectId);
								const projectPath = project?.path;
								if (projectPath) {
									for (let change of item.changes) {
										const path = getEditorUri({
											schemeId: defaultCodeEditor.current.schemeIdentifer,
											path: [vscodePath(projectPath), change.path],
										});
										urlService.openExternalUrl(path);
									}
								}
							} catch {
								chipToasts.error("Failed to open in editor");
								console.error("Failed to open in editor");
							}
						}}
					/>
				{/if}
				{#if itemPath}
					<ContextMenuItem
						label={showInFolderLabel}
						icon="open-in-folder"
						onclick={async () => {
							menuOpen = false;
							const project = await projectService.fetchProject(projectId);
							const projectPath = project?.path;
							if (projectPath) {
								const absPath = await backend.joinPath(projectPath, itemPath);
								await fileService.showFileInFolder(absPath);
							}
						}}
					/>
				{/if}
			</ContextMenuSection>
		{:else}
			<ContextMenuSection>
				<p class="text-13">'Woops! Malformed data :(</p>
			</ContextMenuSection>
		{/if}
	</ContextMenu>
{/if}

<DiscardChangesModal bind:this={discardModal} {projectId} {selectionId} />
<StashIntoBranchModal bind:this={stashModal} {projectId} />
<AbsorbPlanModal bind:this={absorbModal} {projectId} {stackId} />

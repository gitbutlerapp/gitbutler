<!-- This is a V3 replacement for `FileContextMenu.svelte` -->
<script lang="ts">
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import AbsorbPlanModal from "$components/stack/AbsorbPlanModal.svelte";
	import DiscardChangesModal from "$components/workspace/DiscardChangesModal.svelte";
	import StashIntoBranchModal from "$components/workspace/StashIntoBranchModal.svelte";
	import { ACTION_SERVICE } from "$lib/actions/actionService.svelte";
	import { AI_SERVICE } from "$lib/ai/service";
	import { BACKEND } from "$lib/backend";
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { getEditorUri, URL_SERVICE } from "$lib/backend/url";
	import { changesToDiffSpec } from "$lib/commits/utils";
	import { projectAiExperimentalFeaturesEnabled, projectAiGenEnabled } from "$lib/config/config";
	import { FILE_SERVICE } from "$lib/files/fileService";
	import { isTreeChange, type TreeChange } from "$lib/hunks/change";
	import { vscodePath } from "$lib/project/project";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
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

	const DEFAULT_MODEL = "gpt-4";

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
	const idSelection = inject(FILE_SELECTION_MANAGER);
	const aiService = inject(AI_SERVICE);
	const actionService = inject(ACTION_SERVICE);
	const fileService = inject(FILE_SERVICE);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const backend = inject(BACKEND);
	const [autoCommit, autoCommitting] = actionService.autoCommit;
	const [branchChanges, branchingChanges] = actionService.branchChanges;
	const [, absorbingChanges] = stackService.absorb;
	const [splitOffChanges] = stackService.splitBranch;
	const [splitBranchIntoDependentBranch] = stackService.splitBranchIntoDependentBranch;

	const projectService = inject(PROJECTS_SERVICE);

	const userSettings = inject(SETTINGS);
	const isUncommitted = $derived(selectionId.type === "worktree");
	const isBranchFiles = $derived(selectionId.type === "branch");
	const selectionBranchName = $derived(
		selectionId.type === "branch" ? selectionId.branchName : undefined,
	);

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

	let contextMenu: ReturnType<typeof ContextMenu>;
	let discardModal: ReturnType<typeof DiscardChangesModal>;
	let stashModal: ReturnType<typeof StashIntoBranchModal>;
	let absorbModal: ReturnType<typeof AbsorbPlanModal>;
	let aiConfigurationValid = $state(false);

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));
	const experimentalFeaturesEnabled = $derived(projectAiExperimentalFeaturesEnabled(projectId));

	const canUseGBAI = $derived(
		$aiGenEnabled && aiConfigurationValid && $experimentalFeaturesEnabled,
	);

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

	export function open(e: MouseEvent | HTMLElement, item: ChangedFilesItem) {
		contextMenu.open(e, item);
		aiService.validateGitButlerAPIConfiguration().then((value) => {
			aiConfigurationValid = value;
		});
	}

	export function close() {
		contextMenu.close();
	}

	async function uncommitChanges(stackId: string, commitId: string, changes: TreeChange[]) {
		const { replacedCommits } = await stackService.uncommitChanges({
			projectId,
			stackId,
			commitId,
			changes: changesToDiffSpec(changes),
		});
		const newCommitId = replacedCommits.find(([before]) => before === commitId)?.[1];
		const branchName = uiState.lane(stackId).selection.current?.branchName;
		const selectedFiles = changes.map((change) => ({ ...selectionId, path: change.path }));

		// Unselect the uncommitted files
		idSelection.removeMany(selectedFiles);

		if (newCommitId && branchName) {
			const previewOpen = uiState.lane(stackId).selection.current?.previewOpen ?? false;
			// Update the selection to the new commit
			uiState.lane(stackId).selection.set({ branchName, commitId: newCommitId, previewOpen });
		}
		contextMenu.close();
	}

	async function triggerAutoCommit(changes: TreeChange[]) {
		try {
			uiState.global.modal.set({
				type: "auto-commit",
				projectId,
			});
			await autoCommit({
				projectId,
				target: {
					type: "treeChanges",
					subject: { changes, assigned_stack_id: stackId ?? null },
				},
				useAi: $aiGenEnabled,
			});
		} catch (error) {
			console.error("Auto commit failed:", error);
			uiState.global.modal.set(undefined);
		}
	}

	async function triggerBranchChanges(changes: TreeChange[]) {
		if (!canUseGBAI) {
			chipToasts.error("GitButler AI is not configured or enabled for this project.");
			return;
		}

		try {
			await chipToasts.promise(branchChanges({ projectId, changes, model: DEFAULT_MODEL }), {
				loading: "Creating a branch and committing changes",
				success: "Branching changes succeeded",
				error: "Branching changes failed",
			});
		} catch (error) {
			console.error("Branching changes failed:", error);
		}
	}

	async function split(changes: TreeChange[]) {
		if (!stackId) {
			chipToasts.error("No stack selected to split off changes.");
			return;
		}

		if (selectionId.type !== "branch") {
			chipToasts.error("Please select a branch to split off changes.");
			return;
		}

		const branchName = selectionId.branchName;

		const fileNames = changes.map((change) => change.path);

		try {
			await chipToasts.promise(
				(async () => {
					const newBranchName = await stackService.fetchNewBranchName(projectId);

					if (!newBranchName) {
						throw new Error("Failed to generate a new branch name.");
					}

					await splitOffChanges({
						projectId,
						sourceStackId: stackId,
						sourceBranchName: branchName,
						fileChangesToSplitOff: fileNames,
						newBranchName: newBranchName,
					});
				})(),
				{
					loading: "Splitting off changes",
					success: "Changes split off into a new branch",
					error: "Failed to split off changes",
				},
			);
		} catch (error) {
			console.error("Failed to split off changes:", error);
		}
	}

	async function splitIntoDependentBranch(changes: TreeChange[]) {
		if (!stackId) {
			chipToasts.error("No stack selected to split off changes.");
			return;
		}

		if (selectionId.type !== "branch") {
			chipToasts.error("Please select a branch to split off changes.");
			return;
		}

		const branchName = selectionId.branchName;
		const fileNames = changes.map((change) => change.path);

		try {
			await chipToasts.promise(
				(async () => {
					const newBranchName = await stackService.fetchNewBranchName(projectId);

					if (!newBranchName) {
						throw new Error("Failed to generate a new branch name.");
					}

					await splitBranchIntoDependentBranch({
						projectId,
						sourceStackId: stackId,
						sourceBranchName: branchName,
						fileChangesToSplitOff: fileNames,
						newBranchName: newBranchName,
					});
				})(),
				{
					loading: "Splitting into dependent branch",
					success: "Changes split into a dependent branch",
					error: "Failed to split into dependent branch",
				},
			);
		} catch (error) {
			console.error("Failed to split into dependent branch:", error);
		}
	}
</script>

<ContextMenu
	bind:this={contextMenu}
	{leftClickTrigger}
	rightClickTrigger={trigger}
	side="bottom"
	{align}
	{onopen}
	{onclose}
>
	{#snippet children(item: unknown)}
		{#if isChangedFilesItem(item)}
			{@const deletion = isDeleted(item)}
			{@const itemPath = getItemPath(item)}
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
								contextMenu.close();
							}}
						/>
						<ContextMenuItem
							label="Stash into branch…"
							icon="branch-bottom-up-arrow"
							onclick={() => {
								stashModal.show(item);
								contextMenu.close();
							}}
						/>
						<ContextMenuItem
							label="Absorb changes"
							icon="commit-absorb"
							testId={TestId.FileListItemContextMenu_Absorb}
							onclick={() => {
								absorbModal.show(item.changes);
								contextMenu.close();
							}}
							disabled={absorbingChanges.current.isLoading}
						/>
						<ContextMenuItem
							label="Auto commit"
							icon="commit-ai"
							onclick={async () => {
								contextMenu.close();
								triggerAutoCommit(changes);
							}}
							disabled={autoCommitting.current.isLoading}
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

					{#if isBranchFiles && stackId && selectionBranchName}
						{@const branchIsConflicted = stackService.isBranchConflicted(
							projectId,
							stackId,
							selectionBranchName,
						)}
						<ReduxResult {projectId} result={branchIsConflicted?.result}>
							{#snippet children(isConflicted)}
								{#if isConflicted === false}
									<ContextMenuItem
										label="Split off changes"
										icon="split"
										onclick={() => {
											split(changes);
											contextMenu.close();
										}}
									/>
									<ContextMenuItem
										label="Split into dependent branch"
										icon="stack-plus"
										onclick={() => {
											splitIntoDependentBranch(changes);
											contextMenu.close();
										}}
									/>
								{/if}
							{/snippet}
						</ReduxResult>
					{/if}
				</ContextMenuSection>
			{/if}

			{#if itemPath}
				<ContextMenuSection>
					<ContextMenuItemSubmenu label="Copy path" icon="copy">
						{#snippet submenu({ close: closeSubmenu })}
							<ContextMenuSection>
								<ContextMenuItem
									label="Copy path"
									onclick={async () => {
										const project = await projectService.fetchProject(projectId);
										const projectPath = project?.path;
										if (projectPath) {
											const absPath = await backend.joinPath(projectPath, itemPath);

											await clipboardService.write(absPath, {
												message: "Absolute path copied",
												errorMessage: "Failed to copy absolute path",
											});
										}
										closeSubmenu();
										contextMenu.close();
									}}
								/>
								<ContextMenuItem
									label="Copy relative path"
									onclick={async () => {
										await clipboardService.write(itemPath, {
											message: "Relative path copied",
											errorMessage: "Failed to copy relative path",
										});
										closeSubmenu();
										contextMenu.close();
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
						label="Open in {$userSettings.defaultCodeEditor.displayName}"
						icon="open-in-ide"
						disabled={deletion}
						onclick={async () => {
							try {
								const project = await projectService.fetchProject(projectId);
								const projectPath = project?.path;
								if (projectPath) {
									for (let change of item.changes) {
										const path = getEditorUri({
											schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
											path: [vscodePath(projectPath), change.path],
										});
										urlService.openExternalUrl(path);
									}
								}
								contextMenu.close();
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
							const project = await projectService.fetchProject(projectId);
							const projectPath = project?.path;
							if (projectPath) {
								const absPath = await backend.joinPath(projectPath, itemPath);
								await fileService.showFileInFolder(absPath);
							}
							contextMenu.close();
						}}
					/>
				{/if}
			</ContextMenuSection>

			{#if canUseGBAI && isUncommitted}
				<ContextMenuSection>
					<ContextMenuItemSubmenu label="Experimental AI" icon="lab">
						{#snippet submenu({ close: closeSubmenu })}
							<ContextMenuSection>
								<ContextMenuItem
									label="Branch changes"
									onclick={() => {
										closeSubmenu();
										contextMenu.close();
										triggerBranchChanges(item.changes);
									}}
									disabled={branchingChanges.current.isLoading}
								/>
							</ContextMenuSection>
						{/snippet}
					</ContextMenuItemSubmenu>
				</ContextMenuSection>
			{/if}
		{:else}
			<ContextMenuSection>
				<p class="text-13">'Woops! Malformed data :(</p>
			</ContextMenuSection>
		{/if}
	{/snippet}
</ContextMenu>

<DiscardChangesModal bind:this={discardModal} {projectId} {selectionId} />
<StashIntoBranchModal bind:this={stashModal} {projectId} />
<AbsorbPlanModal bind:this={absorbModal} {projectId} {stackId} />

<!-- This is a V3 replacement for `FileContextMenu.svelte` -->
<script lang="ts">
	import BranchNameTextbox from '$components/BranchNameTextbox.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { ACTION_SERVICE } from '$lib/actions/actionService.svelte';
	import { AI_SERVICE } from '$lib/ai/service';
	import { BACKEND } from '$lib/backend';
	import { CLIPBOARD_SERVICE } from '$lib/backend/clipboard';
	import { changesToDiffSpec } from '$lib/commits/utils';
	import { projectAiExperimentalFeaturesEnabled, projectAiGenEnabled } from '$lib/config/config';
	import { autoSelectBranchCreationFeature } from '$lib/config/uiFeatureFlags';
	import { FILE_SERVICE } from '$lib/files/fileService';
	import { isTreeChange, type TreeChange } from '$lib/hunks/change';
	import { vscodePath } from '$lib/project/project';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { getEditorUri, URL_SERVICE } from '$lib/utils/url';
	import { inject } from '@gitbutler/core/context';
	import {
		AsyncButton,
		Button,
		ContextMenu,
		ContextMenuItem,
		ContextMenuItemSubmenu,
		ContextMenuSection,
		FileListItem,
		Modal,
		chipToasts
	} from '@gitbutler/ui';

	import type { SelectionId } from '$lib/selection/key';

	type Props = {
		projectId: string;
		stackId: string | undefined;
		selectionId: SelectionId;
		trigger?: HTMLElement;
		editMode?: boolean;
	};

	type ChangedFilesItem = {
		changes: TreeChange[];
	};

	function isChangedFilesItem(item: unknown): item is ChangedFilesItem {
		return (
			typeof item === 'object' &&
			item !== null &&
			'changes' in item &&
			Array.isArray(item.changes) &&
			item.changes.every(isTreeChange)
		);
	}

	type ChangedFolderItem = ChangedFilesItem & {
		path: string;
	};

	function isChangedFolderItem(item: ChangedFilesItem): item is ChangedFolderItem {
		return 'path' in item && typeof item.path === 'string';
	}

	const { trigger, selectionId, stackId, projectId, editMode = false }: Props = $props();
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
	const [absorbChanges, absorbingChanges] = actionService.absorb;
	const [splitOffChanges] = stackService.splitBranch;
	const [splitBranchIntoDependentBranch] = stackService.splitBrancIntoDependentBranch;

	const projectService = inject(PROJECTS_SERVICE);

	const userSettings = inject(SETTINGS);
	const isUncommitted = $derived(selectionId.type === 'worktree');
	const isBranchFiles = $derived(selectionId.type === 'branch');
	const selectionBranchName = $derived(
		selectionId.type === 'branch' ? selectionId.branchName : undefined
	);

	// Platform-specific label for "Show in Finder/Explorer/File Manager"
	const showInFolderLabel = (() => {
		switch (backend.platformName) {
			case 'macos':
				return 'Show in Finder';
			case 'windows':
				return 'Show in Explorer';
			default:
				return 'Show in File Manager';
		}
	})();

	let confirmationModal: ReturnType<typeof Modal> | undefined;
	let stashConfirmationModal: ReturnType<typeof Modal> | undefined;
	let contextMenu: ReturnType<typeof ContextMenu>;
	let aiConfigurationValid = $state(false);

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));
	const experimentalFeaturesEnabled = $derived(projectAiExperimentalFeaturesEnabled(projectId));

	const canUseGBAI = $derived(
		$aiGenEnabled && aiConfigurationValid && $experimentalFeaturesEnabled
	);

	function isDeleted(item: ChangedFilesItem): boolean {
		if (isChangedFolderItem(item)) {
			return false;
		}
		return item.changes.some((change) => {
			return change.status.type === 'Deletion';
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

	async function confirmDiscard(item: ChangedFilesItem) {
		await stackService.discardChanges({
			projectId,
			worktreeChanges: changesToDiffSpec(item.changes)
		});

		const selectedFiles = item.changes.map((change) => ({ ...selectionId, path: change.path }));

		// Unselect the discarded files
		idSelection.removeMany(selectedFiles);

		confirmationModal?.close();
	}

	let stashBranchName = $state<string>();
	let slugifiedRefName: string | undefined = $state();
	let stashBranchNameInput = $state<ReturnType<typeof BranchNameTextbox>>();

	async function confirmStashIntoBranch(item: ChangedFilesItem, branchName: string | undefined) {
		if (!branchName) {
			return;
		}

		await stackService.stashIntoBranch({
			projectId,
			branchName,
			worktreeChanges: changesToDiffSpec(item.changes)
		});

		stashConfirmationModal?.close();
	}

	export function open(e: MouseEvent, item: ChangedFilesItem) {
		contextMenu.open(e, item);
		aiService.validateGitButlerAPIConfiguration().then((value) => {
			aiConfigurationValid = value;
		});
	}

	async function uncommitChanges(stackId: string, commitId: string, changes: TreeChange[]) {
		const { replacedCommits } = await stackService.uncommitChanges({
			projectId,
			stackId,
			commitId,
			changes: changesToDiffSpec(changes)
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
		if (!canUseGBAI) {
			chipToasts.error('GitButler AI is not configured or enabled for this project.');
			return;
		}

		try {
			await chipToasts.promise(autoCommit({ projectId, changes }), {
				loading: 'Started auto commit',
				success: 'Auto commit succeeded',
				error: 'Auto commit failed'
			});
		} catch (error) {
			console.error('Auto commit failed:', error);
		}
	}

	async function triggerBranchChanges(changes: TreeChange[]) {
		if (!canUseGBAI) {
			chipToasts.error('GitButler AI is not configured or enabled for this project.');
			return;
		}

		try {
			await chipToasts.promise(branchChanges({ projectId, changes }), {
				loading: 'Creating a branch and committing changes',
				success: 'Branching changes succeeded',
				error: 'Branching changes failed'
			});
		} catch (error) {
			console.error('Branching changes failed:', error);
		}
	}

	async function triggerAbsorbChanges(changes: TreeChange[]) {
		if (!canUseGBAI) {
			chipToasts.error('GitButler AI is not configured or enabled for this project.');
			return;
		}

		try {
			await chipToasts.promise(absorbChanges({ projectId, changes }), {
				loading: 'Looking for the best place to absorb the changes',
				success: 'Absorbing changes succeeded',
				error: 'Absorbing changes failed'
			});
		} catch (error) {
			console.error('Absorbing changes failed:', error);
		}
	}

	async function split(changes: TreeChange[]) {
		if (!stackId) {
			chipToasts.error('No stack selected to split off changes.');
			return;
		}

		if (selectionId.type !== 'branch') {
			chipToasts.error('Please select a branch to split off changes.');
			return;
		}

		const branchName = selectionId.branchName;

		const fileNames = changes.map((change) => change.path);

		try {
			await chipToasts.promise(
				(async () => {
					const newBranchName = await stackService.fetchNewBranchName(projectId);

					if (!newBranchName) {
						throw new Error('Failed to generate a new branch name.');
					}

					await splitOffChanges({
						projectId,
						sourceStackId: stackId,
						sourceBranchName: branchName,
						fileChangesToSplitOff: fileNames,
						newBranchName: newBranchName
					});
				})(),
				{
					loading: 'Splitting off changes',
					success: 'Changes split off into a new branch',
					error: 'Failed to split off changes'
				}
			);
		} catch (error) {
			console.error('Failed to split off changes:', error);
		}
	}

	async function splitIntoDependentBranch(changes: TreeChange[]) {
		if (!stackId) {
			chipToasts.error('No stack selected to split off changes.');
			return;
		}

		if (selectionId.type !== 'branch') {
			chipToasts.error('Please select a branch to split off changes.');
			return;
		}

		const branchName = selectionId.branchName;
		const fileNames = changes.map((change) => change.path);

		try {
			await chipToasts.promise(
				(async () => {
					const newBranchName = await stackService.fetchNewBranchName(projectId);

					if (!newBranchName) {
						throw new Error('Failed to generate a new branch name.');
					}

					await splitBranchIntoDependentBranch({
						projectId,
						sourceStackId: stackId,
						sourceBranchName: branchName,
						fileChangesToSplitOff: fileNames,
						newBranchName: newBranchName
					});
				})(),
				{
					loading: 'Splitting into dependent branch',
					success: 'Changes split into a dependent branch',
					error: 'Failed to split into dependent branch'
				}
			);
		} catch (error) {
			console.error('Failed to split into dependent branch:', error);
		}
	}
</script>

<ContextMenu bind:this={contextMenu} rightClickTrigger={trigger}>
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
							icon="bin"
							onclick={() => {
								confirmationModal?.show(item);
								contextMenu.close();
							}}
						/>
					{/if}
					{#if isUncommitted}
						<ContextMenuItem
							label="Stash into branch…"
							icon="stash"
							onclick={async () => {
								stashConfirmationModal?.show(item);
								stashBranchName = await stackService.fetchNewBranchName(projectId);
								// Select text after async value is loaded and DOM is updated
								if ($autoSelectBranchCreationFeature) {
									await stashBranchNameInput?.selectAll();
								}
								contextMenu.close();
							}}
						/>
					{/if}
					{#if selectionId.type === 'commit' && stackId && !editMode}
						{@const commitId = selectionId.commitId}
						<ContextMenuItem
							label="Uncommit changes"
							icon="undo-small"
							onclick={async () => uncommitChanges(stackId, commitId, changes)}
						/>
					{/if}

					{#if isBranchFiles && stackId && selectionBranchName}
						{@const branchIsConflicted = stackService.isBranchConflicted(
							projectId,
							stackId,
							selectionBranchName
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
										icon="new-dep-branch"
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
												message: 'Absolute path copied',
												errorMessage: 'Failed to copy absolute path'
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
											message: 'Relative path copied',
											errorMessage: 'Failed to copy relative path'
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
						icon="open-editor"
						disabled={deletion}
						onclick={async () => {
							try {
								const project = await projectService.fetchProject(projectId);
								const projectPath = project?.path;
								if (projectPath) {
									for (let change of item.changes) {
										const path = getEditorUri({
											schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
											path: [vscodePath(projectPath), change.path]
										});
										urlService.openExternalUrl(path);
									}
								}
								contextMenu.close();
							} catch {
								chipToasts.error('Failed to open in editor');
								console.error('Failed to open in editor');
							}
						}}
					/>
				{/if}
				{#if itemPath}
					<ContextMenuItem
						label={showInFolderLabel}
						icon="open-folder"
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
									label="Auto commit"
									tooltip="Try to figure out where to commit the changes. Can create new branches too."
									onclick={async () => {
										closeSubmenu();
										contextMenu.close();
										triggerAutoCommit(item.changes);
									}}
									disabled={autoCommitting.current.isLoading}
								/>
								<ContextMenuItem
									label="Branch changes"
									tooltip="Create a new branch and commit the changes into it."
									onclick={() => {
										closeSubmenu();
										contextMenu.close();
										triggerBranchChanges(item.changes);
									}}
									disabled={branchingChanges.current.isLoading}
								/>
								<ContextMenuItem
									label="Absorb changes"
									tooltip="Try to find the best place to absorb the changes into."
									onclick={() => {
										closeSubmenu();
										contextMenu.close();
										triggerAbsorbChanges(item.changes);
									}}
									disabled={absorbingChanges.current.isLoading}
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

<Modal
	width="small"
	type="warning"
	title="Discard changes"
	bind:this={confirmationModal}
	onSubmit={(_, item) => isChangedFilesItem(item) && confirmDiscard(item)}
>
	{#snippet children(item)}
		{#if isChangedFilesItem(item)}
			{#if isChangedFolderItem(item)}
				<p class="discard-caption">
					Are you sure you want to discard all changes in
					<span class="text-bold">{item.path}</span>?
				</p>
			{:else}
				{@const changes = item.changes}
				{#if changes.length < 10}
					<p class="discard-caption">
						Are you sure you want to discard the changes<br />to the following files:
					</p>
					<ul class="file-list">
						{#each changes as change, i}
							<FileListItem
								filePath={change.path}
								fileStatus={computeChangeStatus(change)}
								clickable={false}
								listMode="list"
								hideBorder={i === changes.length - 1}
							/>
						{/each}
					</ul>
				{:else}
					<p>
						Discard the changes to all <span class="text-bold">
							{changes.length} files
						</span>?
					</p>
				{/if}
			{/if}
		{:else}
			<p class="text-13">Woops! Malformed data :(</p>
		{/if}
	{/snippet}
	{#snippet controls(close, item)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<AsyncButton style="danger" type="submit" action={async () => await confirmDiscard(item)}>
			Confirm
		</AsyncButton>
	{/snippet}
</Modal>

<Modal
	width={434}
	type="info"
	title="Stash changes into a new branch"
	bind:this={stashConfirmationModal}
	onSubmit={(_, item) => isChangedFilesItem(item) && confirmStashIntoBranch(item, slugifiedRefName)}
>
	{#snippet children(item)}
		<div class="content-wrap">
			<BranchNameTextbox
				bind:this={stashBranchNameInput}
				id="stashBranchName"
				placeholder="Enter your branch name..."
				bind:value={stashBranchName}
				autofocus
				onslugifiedvalue={(value) => (slugifiedRefName = value)}
			/>
			<div class="explanation">
				<p class="primary-text">
					{#if isChangedFolderItem(item)}
						All changes in this folder
					{:else}
						Your selected changes
					{/if}
					will be moved to a new branch and removed from your current workspace. To get these changes
					back later, switch to the new branch and uncommit the stash.
				</p>
			</div>

			<div class="technical-note">
				<p class="text-12 text-body clr-text-2">
					💡 This creates a new branch, commits your changes, then unapplies the branch. Future
					versions will have simpler stash management.
				</p>
			</div>
		</div>
	{/snippet}
	{#snippet controls(close, item)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<AsyncButton
			style="pop"
			disabled={!slugifiedRefName}
			type="submit"
			action={async () => await confirmStashIntoBranch(item, slugifiedRefName)}
		>
			Stash into branch
		</AsyncButton>
	{/snippet}
</Modal>

<style lang="postcss">
	.discard-caption {
		color: var(--clr-text-2);
	}
	.file-list {
		display: flex;
		flex-direction: column;
		margin-top: 12px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}
	/* MODAL WINDOW */
	.content-wrap {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
</style>

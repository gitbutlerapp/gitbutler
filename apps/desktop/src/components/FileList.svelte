<!-- This is a V3 replacement for `BranchFileList.svelte` -->
<script lang="ts">
	import EditPatchConfirmModal from '$components/EditPatchConfirmModal.svelte';
	import FileListItemWrapper from '$components/FileListItemWrapper.svelte';
	import FileTreeNode from '$components/FileTreeNode.svelte';
	import LazyList from '$components/LazyList.svelte';
	import { ACTION_SERVICE } from '$lib/actions/actionService.svelte';
	import { AI_SERVICE } from '$lib/ai/service';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { conflictEntryHint } from '$lib/conflictEntryPresence';
	import {
		getLockedCommitIds,
		getLockedTargets,
		isFileLocked
	} from '$lib/dependencies/dependencies';
	import { DEPENDENCY_SERVICE } from '$lib/dependencies/dependencyService.svelte';
	import { editPatch } from '$lib/editMode/editPatchUtils';
	import { abbreviateFolders, changesToFileTree } from '$lib/files/filetreeV3';
	import { type TreeChange, isExecutableStatus } from '$lib/hunks/change';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { showToast } from '$lib/notifications/toasts';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { selectFilesInList, updateSelection } from '$lib/selection/fileSelectionUtils';
	import { type SelectionId } from '$lib/selection/key';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { inject, injectOptional } from '@gitbutler/core/context';
	import { AsyncButton, FileListItem, TestId } from '@gitbutler/ui';
	import { FOCUS_MANAGER } from '@gitbutler/ui/focus/focusManager';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { untrack } from 'svelte';
	import { get } from 'svelte/store';
	import type { ConflictEntriesObj } from '$lib/files/conflicts';

	const DEFAULT_MODEL = 'gpt-4.1';

	type Props = {
		projectId: string;
		stackId?: string;
		changes: TreeChange[];
		listMode: 'list' | 'tree';
		showCheckboxes?: boolean;
		selectionId: SelectionId;
		conflictEntries?: ConflictEntriesObj;
		draggableFiles?: boolean;
		ancestorMostConflictedCommitId?: string;
		onFileClick?: (index: number) => void;
		allowUnselect?: boolean;
		showLockedIndicator?: boolean;
		dataTestId?: string;
	};

	const {
		projectId,
		changes,
		listMode,
		selectionId,
		showCheckboxes,
		stackId,
		conflictEntries,
		draggableFiles,
		ancestorMostConflictedCommitId,
		onFileClick,
		allowUnselect = true,
		showLockedIndicator = false,
		dataTestId
	}: Props = $props();

	const focusManager = inject(FOCUS_MANAGER);
	const idSelection = inject(FILE_SELECTION_MANAGER);
	const aiService = inject(AI_SERVICE);
	const actionService = inject(ACTION_SERVICE);
	const modeService = injectOptional(MODE_SERVICE, undefined);
	const dependencyService = inject(DEPENDENCY_SERVICE);
	const userSettings = inject(SETTINGS);

	const [autoCommit] = actionService.autoCommit;
	const [branchChanges] = actionService.branchChanges;

	let editPatchModal: EditPatchConfirmModal | undefined = $state();
	let selectedFilePath = $state('');

	const filePaths = $derived(changes.map((change) => change.path));
	const fileDependenciesQuery = $derived(
		showLockedIndicator ? dependencyService.filesDependencies(projectId, filePaths) : null
	);
	const fileDependencies = $derived(fileDependenciesQuery?.result.data || []);

	function showEditPatchConfirmation(filePath: string) {
		selectedFilePath = filePath;
		editPatchModal?.show();
	}

	function handleConfirmEditPatch() {
		editPatchModal?.hide();
		editPatch({
			modeService,
			commitId: ancestorMostConflictedCommitId!,
			stackId: stackId!,
			projectId
		});
	}

	function handleCancelEditPatch() {
		editPatchModal?.hide();
		selectedFilePath = '';
	}

	let aiConfigurationValid = $state(false);
	let active = $state(false);

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));

	const canUseGBAI = $derived(aiGenEnabled && aiConfigurationValid);
	const selectedFileIds = $derived(idSelection.values(selectionId));

	$effect(() => {
		aiService.validateGitButlerAPIConfiguration().then((value) => {
			aiConfigurationValid = value;
		});
	});

	/**
	 * Create a branch and commit from the selected changes.
	 *
	 * _Branch [/bræntʃ/]_ is a verb that means to create a new branch and commit from the current changes.
	 *
	 * _According to who? Me._
	 *
	 * - Anonymous
	 */
	async function branchSelection() {
		const selectedFiles = idSelection.values(selectionId);
		if (selectionId.type !== 'worktree' || selectedFiles.length === 0 || !canUseGBAI) return;

		showToast({
			style: 'info',
			title: 'Creating a branch and committing the changes',
			message: 'This may take a few seconds.'
		});

		const treeChanges = changes.filter((change) =>
			selectedFiles.some((file) => file.path === change.path)
		);

		await branchChanges({
			projectId,
			changes: treeChanges,
			model: DEFAULT_MODEL
		});

		showToast({
			style: 'success',
			title: 'And... done!',
			message: `Now, you're free to continue`
		});
	}

	async function autoCommitSelection() {
		const selectedFiles = idSelection.values(selectionId);
		if (selectionId.type !== 'worktree' || selectedFiles.length === 0 || !canUseGBAI) return;

		showToast({
			style: 'info',
			title: 'Figuring out where to commit the changes',
			message: 'This may take a few seconds.'
		});

		const treeChanges = changes.filter((change) =>
			selectedFiles.some((file) => file.path === change.path)
		);

		await autoCommit({
			projectId,
			changes: treeChanges,
			model: DEFAULT_MODEL
		});

		showToast({
			style: 'success',
			title: 'And... done!',
			message: `Now, you're free to continue`
		});
	}

	const unrepresentedConflictedEntries = $derived.by(() => {
		if (!conflictEntries?.entries) return {};

		return Object.fromEntries(
			Object.entries(conflictEntries.entries).filter(([key, _value]) =>
				changes.every((change) => change.path !== key)
			)
		);
	});

	function handleKeyDown(change: TreeChange, idx: number, e: KeyboardEvent) {
		if (e.key === 'Enter' || e.key === ' ' || e.key === 'l') {
			e.stopPropagation();
			selectFilesInList(
				e,
				change,
				changes,
				idSelection,
				selectedFileIds,
				true,
				idx,
				selectionId,
				allowUnselect
			);
			onFileClick?.(idx);
			return true;
		}

		if (e.code === 'KeyB' && (e.ctrlKey || e.metaKey) && e.altKey) {
			branchSelection();
			e.preventDefault();
			return;
		}

		if (e.code === 'KeyC' && (e.ctrlKey || e.metaKey) && e.altKey) {
			autoCommitSelection();
			e.preventDefault();
			return;
		}

		if (
			updateSelection({
				allowMultiple: true,
				ctrlKey: e.ctrlKey,
				metaKey: e.metaKey,
				shiftKey: e.shiftKey,
				key: e.key,
				targetElement: e.currentTarget as HTMLElement,
				files: changes,
				selectedFileIds,
				fileIdSelection: idSelection,
				selectionId: selectionId,
				preventDefault: () => e.preventDefault()
			})
		) {
			const lastAdded = get(idSelection.getById(selectionId).lastAdded);
			if (lastAdded) {
				onFileClick?.(lastAdded.index);
			}
		}
	}

	const currentSelection = $derived(idSelection.getById(selectionId));
	const lastAdded = $derived(currentSelection.lastAdded);
	const lastAddedIndex = $derived($lastAdded?.index);
	$effect(() => {
		if (lastAddedIndex) {
			untrack(() => {
				if (active) {
					focusManager.focusNthSibling(lastAddedIndex);
				}
			});
		}
	});
</script>

{#snippet fileTemplate(change: TreeChange, idx: number, depth: number = 0, isLast: boolean = false)}
	{@const isExecutable = isExecutableStatus(change.status)}
	{@const selected = idSelection.has(change.path, selectionId)}
	{@const locked = showLockedIndicator && isFileLocked(change.path, fileDependencies)}
	{@const lockedCommitIds = showLockedIndicator
		? getLockedCommitIds(change.path, fileDependencies)
		: []}
	{@const lockedTargets = showLockedIndicator
		? getLockedTargets(change.path, fileDependencies)
		: []}
	<FileListItemWrapper
		{selectionId}
		{change}
		{projectId}
		{stackId}
		{selected}
		{listMode}
		{depth}
		{active}
		{locked}
		{lockedCommitIds}
		{lockedTargets}
		{isLast}
		draggable={draggableFiles}
		executable={isExecutable}
		showCheckbox={showCheckboxes}
		focusableOpts={{
			onKeydown: (e) => handleKeyDown(change, idx, e),
			focusable: true
		}}
		onclick={(e) => {
			e.stopPropagation();
			selectFilesInList(
				e,
				change,
				changes,
				idSelection,
				selectedFileIds,
				true,
				idx,
				selectionId,
				allowUnselect
			);
			if (idSelection.has(change.path, selectionId)) {
				onFileClick?.(idx);
			}
		}}
		{conflictEntries}
	/>
{/snippet}

<div
	data-testid={dataTestId}
	class="file-list"
	use:focusable={{
		vertical: true,
		onActive: (value) => (active = value)
	}}
>
	<!-- Conflicted changes -->
	{#if Object.keys(unrepresentedConflictedEntries).length > 0}
		{@const entries = Object.entries(unrepresentedConflictedEntries)}
		<div class="conflicted-entries">
			{#each entries as [path, kind], i}
				<FileListItem
					draggable={draggableFiles}
					filePath={path}
					pathFirst={$userSettings.pathFirst}
					{active}
					conflicted
					conflictHint={conflictEntryHint(kind)}
					listMode="list"
					isLast={!ancestorMostConflictedCommitId && i === entries.length - 1}
					onclick={(e) => {
						e.stopPropagation();
						showEditPatchConfirmation(path);
					}}
				/>
			{/each}

			{#if ancestorMostConflictedCommitId}
				<div class="conflicted-entries__action">
					<p class="text-12 text-body clr-text-2">
						If the branch has multiple conflicted commits, GitButler opens the earliest one first,
						since later commits depend on it.
					</p>
					<AsyncButton
						testId={TestId.CommitDrawerResolveConflictsButton}
						kind="solid"
						style="danger"
						wide
						action={() =>
							editPatch({
								modeService,
								commitId: ancestorMostConflictedCommitId!,
								stackId: stackId!,
								projectId
							})}
					>
						Resolve conflicts
					</AsyncButton>
				</div>
			{/if}
		</div>
	{/if}

	<!-- Other changes -->
	{#if changes.length > 0}
		{#if listMode === 'tree'}
			<!--
				We need to use sortedChanges here because otherwise we will end up
				with incorrect indexes
			-->
			{@const node = abbreviateFolders(changesToFileTree(changes))}
			<FileTreeNode
				isRoot
				{projectId}
				{selectionId}
				{stackId}
				{node}
				{showCheckboxes}
				{draggableFiles}
				{changes}
				{fileTemplate}
			/>
		{:else}
			<LazyList items={changes} chunkSize={100}>
				{#snippet template(change, context)}
					<!--
						There is a bug here related to the reactivity of `idSelection.has`,
						affecting somehow the first item in the list of files.. but only where
						used for the "assigned files" of the workspace.

						This unused variable is a workaround, while present the reactivity
						works as expected.

						TODO: Bisect this issue, it was introduced between nightly version
						0.5.1705 and 0.5.1783.
						-->
					{@const _selected = idSelection.has(change.path, selectionId)}
					{@render fileTemplate(change, context.index, 0, context.last)}
				{/snippet}
			</LazyList>
		{/if}
	{/if}
</div>

<EditPatchConfirmModal
	bind:this={editPatchModal}
	fileName={selectedFilePath}
	onConfirm={handleConfirmEditPatch}
	onCancel={handleCancelEditPatch}
/>

<style lang="postcss">
	.file-list {
		display: flex;
		flex-direction: column;
	}

	.conflicted-entries {
		display: flex;
		flex-direction: column;
	}

	.conflicted-entries__action {
		display: flex;
		flex-direction: column;
		justify-content: center;
		padding: 12px;
		gap: 12px;
		border-bottom: 1px solid var(--clr-border-2);
	}
</style>

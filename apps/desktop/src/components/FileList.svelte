<!-- This is a V3 replacement for `BranchFileList.svelte` -->
<script lang="ts">
	import EditPatchConfirmModal from '$components/EditPatchConfirmModal.svelte';
	import FileListItemWrapper from '$components/FileListItemWrapper.svelte';
	import FileTreeNode from '$components/FileTreeNode.svelte';
	import LazyloadContainer from '$components/LazyloadContainer.svelte';
	import { ACTION_SERVICE } from '$lib/actions/actionService.svelte';
	import { AI_SERVICE } from '$lib/ai/service';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { conflictEntryHint } from '$lib/conflictEntryPresence';
	import { editPatch } from '$lib/editMode/editPatchUtils';
	import { abbreviateFolders, changesToFileTree } from '$lib/files/filetreeV3';
	import { type TreeChange, isExecutableStatus } from '$lib/hunks/change';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { showToast } from '$lib/notifications/toasts';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { selectFilesInList, updateSelection } from '$lib/selection/fileSelectionUtils';
	import { type SelectionId } from '$lib/selection/key';
	import { chunk } from '$lib/utils/array';
	import { inject, injectOptional } from '@gitbutler/core/context';
	import { FileListItem } from '@gitbutler/ui';
	import { FOCUS_MANAGER } from '@gitbutler/ui/focus/focusManager';
	import { focusable } from '@gitbutler/ui/focus/focusable';

	import type { ConflictEntriesObj } from '$lib/files/conflicts';

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
		hideLastFileBorder?: boolean;
		onselect?: () => void;
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
		hideLastFileBorder = true,
		onselect
	}: Props = $props();

	const focusManager = inject(FOCUS_MANAGER);
	const idSelection = inject(FILE_SELECTION_MANAGER);
	const aiService = inject(AI_SERVICE);
	const actionService = inject(ACTION_SERVICE);
	const modeService = injectOptional(MODE_SERVICE, undefined);

	const [autoCommit] = actionService.autoCommit;
	const [branchChanges] = actionService.branchChanges;
	let currentDisplayIndex = $state(0);

	let editPatchModal: EditPatchConfirmModal | undefined = $state();
	let selectedFilePath = $state('');

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

	const fileChunks: TreeChange[][] = $derived(chunk(changes, 100));
	const visibleFiles: TreeChange[] = $derived(fileChunks.slice(0, currentDisplayIndex + 1).flat());
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
			style: 'neutral',
			title: 'Creating a branch and committing the changes',
			message: 'This may take a few seconds.'
		});

		const treeChanges = changes.filter((change) =>
			selectedFiles.some((file) => file.path === change.path)
		);

		await branchChanges({
			projectId,
			changes: treeChanges
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
			style: 'neutral',
			title: 'Figuring out where to commit the changes',
			message: 'This may take a few seconds.'
		});

		const treeChanges = changes.filter((change) =>
			selectedFiles.some((file) => file.path === change.path)
		);

		await autoCommit({
			projectId,
			changes: treeChanges
		});

		showToast({
			style: 'success',
			title: 'And... done!',
			message: `Now, you're free to continue`
		});
	}

	function loadMore() {
		if (currentDisplayIndex + 1 >= fileChunks.length) return;
		currentDisplayIndex += 1;
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
			selectFilesInList(e, change, changes, idSelection, selectedFileIds, true, idx, selectionId);
			onselect?.();
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

		if (!e.metaKey) {
			return updateSelection({
				allowMultiple: true,
				metaKey: e.metaKey,
				shiftKey: e.shiftKey,
				key: e.key,
				targetElement: e.currentTarget as HTMLElement,
				files: changes,
				selectedFileIds,
				fileIdSelection: idSelection,
				selectionId: selectionId,
				preventDefault: () => e.preventDefault()
			});
		}
		return false;
	}
	const lastAdded = $derived(idSelection.getById(selectionId).lastAdded);
	$effect(() => {
		if ($lastAdded) focusManager.focusNthSibling($lastAdded.index);
	});
</script>

{#snippet fileTemplate(change: TreeChange, idx: number, depth: number = 0)}
	{@const isExecutable = isExecutableStatus(change.status)}
	{@const selected = idSelection.has(change.path, selectionId)}
	<FileListItemWrapper
		{selectionId}
		{change}
		{projectId}
		{stackId}
		{selected}
		{listMode}
		{depth}
		{active}
		hideBorder={hideLastFileBorder && idx === visibleFiles.length - 1}
		draggable={draggableFiles}
		executable={isExecutable}
		showCheckbox={showCheckboxes}
		focusableOpts={{ onKeydown: (e) => handleKeyDown(change, idx, e), autoAction: true }}
		onclick={(e) => {
			e.stopPropagation();
			selectFilesInList(e, change, changes, idSelection, selectedFileIds, true, idx, selectionId);
			onselect?.();
		}}
		{conflictEntries}
	/>
{/snippet}

<div
	class="file-list"
	use:focusable={{
		vertical: true,
		onActive: (value) => (active = value)
	}}
>
	<!-- Conflicted changes -->
	{#each Object.entries(unrepresentedConflictedEntries) as [path, kind]}
		<FileListItem
			draggable={draggableFiles}
			filePath={path}
			{active}
			conflicted
			conflictHint={conflictEntryHint(kind)}
			listMode="list"
			onclick={(e) => {
				e.stopPropagation();
				showEditPatchConfirmation(path);
			}}
		/>
	{/each}
	<!-- Other changes -->
	{#if visibleFiles.length > 0}
		{#if listMode === 'tree'}
			<!-- We need to use sortedChanges here because otherwise we will end up
		with incorrect indexes -->
			{@const node = abbreviateFolders(changesToFileTree(changes))}
			<FileTreeNode isRoot {stackId} {node} {showCheckboxes} {changes} {fileTemplate} />
		{:else}
			<LazyloadContainer
				minTriggerCount={80}
				ontrigger={() => {
					loadMore();
				}}
				role="listbox"
			>
				{#each visibleFiles as change, idx}
					{@render fileTemplate(change, idx)}
				{/each}
			</LazyloadContainer>
		{/if}
	{/if}
</div>

<EditPatchConfirmModal
	bind:this={editPatchModal}
	fileName={selectedFilePath}
	onConfirm={handleConfirmEditPatch}
	onCancel={handleCancelEditPatch}
/>

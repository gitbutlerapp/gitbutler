<!-- This is a V3 replacement for `BranchFileList.svelte` -->
<script lang="ts">
	import FileListItemWrapper from '$components/FileListItemWrapper.svelte';
	import FileTreeNode from '$components/FileTreeNode.svelte';
	import LazyloadContainer from '$components/LazyloadContainer.svelte';
	import { ACTION_SERVICE } from '$lib/actions/actionService.svelte';
	import { AI_SERVICE } from '$lib/ai/service';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { conflictEntryHint } from '$lib/conflictEntryPresence';
	import { abbreviateFolders, changesToFileTree } from '$lib/files/filetreeV3';
	import { type TreeChange, type Modification } from '$lib/hunks/change';
	import { showToast } from '$lib/notifications/toasts';
	import { ID_SELECTION } from '$lib/selection/idSelection.svelte';
	import { selectFilesInList, updateSelection } from '$lib/selection/idSelectionUtils';
	import { type SelectionId } from '$lib/selection/key';
	import { chunk } from '$lib/utils/array';
	import { inject } from '@gitbutler/shared/context';
	import { FileListItem } from '@gitbutler/ui';

	import type { ConflictEntriesObj } from '$lib/files/conflicts';

	type Props = {
		projectId: string;
		stackId?: string;
		changes: TreeChange[];
		listMode: 'list' | 'tree';
		showCheckboxes?: boolean;
		selectionId: SelectionId;
		active?: boolean;
		conflictEntries?: ConflictEntriesObj;
		draggableFiles?: boolean;
		onselect?: () => void;
	};

	const {
		projectId,
		changes,
		listMode,
		selectionId,
		showCheckboxes,
		active,
		stackId,
		conflictEntries,
		draggableFiles,
		onselect
	}: Props = $props();

	const idSelection = inject(ID_SELECTION);
	const aiService = inject(AI_SERVICE);
	const actionService = inject(ACTION_SERVICE);

	const [autoCommit] = actionService.autoCommit;
	const [branchChanges] = actionService.branchChanges;
	let currentDisplayIndex = $state(0);

	const fileChunks: TreeChange[][] = $derived(chunk(changes, 100));
	const visibleFiles: TreeChange[] = $derived(fileChunks.slice(0, currentDisplayIndex + 1).flat());
	let aiConfigurationValid = $state(false);

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));

	const canUseGBAI = $derived(aiGenEnabled && aiConfigurationValid);

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

	function handleKeyDown(e: KeyboardEvent) {
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

		updateSelection({
			allowMultiple: true,
			metaKey: e.metaKey,
			shiftKey: e.shiftKey,
			key: e.key,
			targetElement: e.currentTarget as HTMLElement,
			files: changes,
			selectedFileIds: idSelection.values(selectionId),
			fileIdSelection: idSelection,
			selectionId: selectionId,
			preventDefault: () => e.preventDefault()
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
</script>

{#snippet fileTemplate(change: TreeChange, idx: number, depth: number = 0)}
	{@const isExecutable = (change.status.subject as Modification).flags}
	<FileListItemWrapper
		{selectionId}
		{change}
		{projectId}
		{stackId}
		{active}
		{listMode}
		{depth}
		draggable={draggableFiles}
		executable={!!isExecutable}
		showCheckbox={showCheckboxes}
		selected={idSelection.has(change.path, selectionId)}
		onclick={(e) => {
			e.stopPropagation();
			selectFilesInList(e, change, changes, idSelection, true, idx, selectionId);
			onselect?.();
		}}
		{conflictEntries}
	/>
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div onkeydown={handleKeyDown}>
	{#each Object.entries(unrepresentedConflictedEntries) as [path, kind]}
		<FileListItem
			draggable={draggableFiles}
			filePath={path}
			conflicted
			conflictHint={conflictEntryHint(kind)}
			listMode="list"
		/>
	{/each}
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

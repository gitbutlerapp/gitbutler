<!-- This is a V3 replacement for `BranchFileList.svelte` -->
<script lang="ts">
	import LazyloadContainer from '$components/LazyloadContainer.svelte';
	import FileListItemWrapper from '$components/v3/FileListItemWrapper.svelte';
	import FileTreeNode from '$components/v3/FileTreeNode.svelte';
	import { PromptService } from '$lib/ai/promptService';
	import { AIService, type DiffInput } from '$lib/ai/service';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { conflictEntryHint } from '$lib/conflictEntryPresence';
	import { abbreviateFolders, changesToFileTree } from '$lib/files/filetreeV3';
	import {
		type TreeChange,
		type Modification,
		previousPathBytesFromTreeChange
	} from '$lib/hunks/change';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { showToast } from '$lib/notifications/toasts';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { selectFilesInList, updateSelection } from '$lib/selection/idSelectionUtils';
	import { type SelectionId } from '$lib/selection/key';
	import StackMacros from '$lib/stacks/macros';
	import {
		StackService,
		type CreateCommitRequestWorktreeChanges
	} from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { chunk } from '$lib/utils/array';
	import { sortLikeFileTree } from '$lib/worktree/changeTree';
	import { inject } from '@gitbutler/shared/context';
	import { isLockfile } from '@gitbutler/shared/lockfiles';
	import FileListItemV3 from '@gitbutler/ui/file/FileListItemV3.svelte';
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
	};

	const {
		projectId,
		changes,
		listMode,
		selectionId,
		showCheckboxes,
		active,
		stackId,
		conflictEntries
	}: Props = $props();

	const [stackService, uiState, idSelection, aiService, promptService, diffService] = inject(
		StackService,
		UiState,
		IdSelection,
		AIService,
		PromptService,
		DiffService
	);

	let currentDisplayIndex = $state(0);

	const fileChunks: TreeChange[][] = $derived(chunk(sortLikeFileTree(changes), 100));
	const visibleFiles: TreeChange[] = $derived(fileChunks.slice(0, currentDisplayIndex + 1).flat());
	const stackMacros = $derived(new StackMacros(projectId, stackService, uiState));

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));
	let aiConfigurationValid = $state(false);

	async function setAIConfigurationValid() {
		aiConfigurationValid = await aiService.validateConfiguration();
	}

	$effect(() => {
		setAIConfigurationValid();
	});

	const canUseAi = $derived(aiGenEnabled && aiConfigurationValid);

	/**
	 * Generate a commit message based on the selected changes.
	 */
	async function generateCommitMessage(
		branchName: string,
		diffInput: DiffInput[]
	): Promise<string | undefined> {
		if (!canUseAi) return;

		const prompt = promptService.selectedCommitPrompt(projectId);
		const output = await aiService.summarizeCommit({
			diffInput,
			useEmojiStyle: false,
			useBriefStyle: false,
			commitTemplate: prompt,
			branchName
		});

		return output;
	}

	/**
	 * Generate a branch name based on the selected changes.
	 */
	async function generateBranchName(diffInput: DiffInput[]): Promise<string | undefined> {
		if (!canUseAi) return;

		const prompt = promptService.selectedBranchPrompt(projectId);
		const newBranchName = await aiService.summarizeBranch({
			type: 'hunks',
			hunks: diffInput,
			branchTemplate: prompt
		});

		return newBranchName;
	}

	/**
	 * Get the diff input for the selected changes.
	 */
	async function getDiffInput(treeChanges: TreeChange[]): Promise<DiffInput[]> {
		const diffInput: DiffInput[] = [];
		const cleanFiles = treeChanges.filter((change) => !isLockfile(change.path));
		const diffs = await diffService.fetchChanges(projectId, cleanFiles);
		for (const diffChange of diffs) {
			const filePath = diffChange.path;
			const diff = diffChange.diff;
			if (diff.type !== 'Patch') continue;

			const diffStringBuffer: string[] = [];

			for (const hunk of diff.subject.hunks) {
				diffStringBuffer.push(hunk.diff);
			}

			const diffString = diffStringBuffer.join('\n');
			diffInput.push({
				filePath,
				diff: diffString
			});
		}

		return diffInput;
	}

	/**
	 * Create a branch and commit from the selected changes.
	 *
	 * _Branch [/bræntʃ/]_ is a verb that means to create a new branch and commit from the current changes.
	 *
	 * _According to who? Me._
	 *
	 * - Anonymous
	 */
	// eslint-disable-next-line @typescript-eslint/no-unused-vars
	async function branchChanges() {
		const selectedFiles = idSelection.values(selectionId);
		if (selectedFiles.length === 0) return;

		showToast({
			style: 'neutral',
			title: 'Creating a branch and commit...',
			message: 'This may take a few seconds.'
		});

		const selectedChanges: CreateCommitRequestWorktreeChanges[] = [];
		const treeChanges = changes.filter((change) =>
			selectedFiles.some((file) => file.path === change.path)
		);
		for (const file of selectedFiles) {
			const change = treeChanges.find((c) => c.path === file.path);
			if (!change) continue;
			const previousPathBytes = previousPathBytesFromTreeChange(change);
			selectedChanges.push({
				pathBytes: change.pathBytes,
				previousPathBytes,
				hunkHeaders: []
			});
		}

		let commitMessage: string | undefined = undefined;
		let branchName: string | undefined = undefined;

		if (canUseAi) {
			const diffInput = await getDiffInput(treeChanges);
			branchName = await generateBranchName(diffInput);

			if (!branchName) {
				showToast({
					style: 'error',
					message: 'Failed to generate branch name.'
				});
				return;
			}

			commitMessage = await generateCommitMessage(branchName, diffInput);
		}

		await stackMacros.branchChanges({
			worktreeChanges: selectedChanges,
			commitMessage,
			branchName
		});

		showToast({
			style: 'success',
			title: 'Branch and commit created successfully.',
			message: `Branch name: ${branchName}`
		});
	}

	function handleKeyDown(e: KeyboardEvent) {
		updateSelection({
			allowMultiple: true,
			metaKey: e.metaKey,
			shiftKey: e.shiftKey,
			key: e.key,
			targetElement: e.currentTarget as HTMLElement,
			files: visibleFiles,
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
		allChanges={changes}
		{projectId}
		{stackId}
		{active}
		{listMode}
		{depth}
		executable={!!isExecutable}
		showCheckbox={showCheckboxes}
		isLast={idx === visibleFiles.length - 1}
		selected={idSelection.has(change.path, selectionId)}
		onclick={(e) => {
			selectFilesInList(e, change, visibleFiles, idSelection, true, idx, selectionId);
		}}
		{conflictEntries}
	/>
{/snippet}

{#each Object.entries(unrepresentedConflictedEntries) as [path, kind]}
	<FileListItemV3
		filePath={path}
		conflicted
		conflictHint={conflictEntryHint(kind)}
		listMode="list"
	/>
{/each}
{#if visibleFiles.length > 0}
	<LazyloadContainer
		minTriggerCount={80}
		ontrigger={() => {
			loadMore();
		}}
		role="listbox"
		onkeydown={handleKeyDown}
	>
		{#if listMode === 'tree'}
			{@const node = abbreviateFolders(changesToFileTree(changes))}
			<FileTreeNode isRoot {node} {showCheckboxes} {changes} {fileTemplate} />
		{:else}
			{#each visibleFiles as change, idx}
				{@render fileTemplate(change, idx)}
			{/each}
		{/if}
	</LazyloadContainer>
{/if}

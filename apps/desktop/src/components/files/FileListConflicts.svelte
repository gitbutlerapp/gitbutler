<!--
	Compound component that renders unrepresented conflict entries
	and the "Resolve conflicts" action. Must be a child of <FileListProvider>.

	Only renders if there are conflict entries not already represented
	in the file list changes.

	Usage:
	```svelte
	<FileListProvider {changes} {selectionId}>
		<FileListConflicts {conflictEntries} {ancestorMostConflictedCommitId} {projectId} />
		<FileListItems mode="list" />
	</FileListProvider>
	```
-->
<script lang="ts">
	import EditPatchConfirmModal from "$components/commit/EditPatchConfirmModal.svelte";
	import { conflictEntryHint } from "$lib/files/conflictEntryPresence";
	import { editPatch } from "$lib/mode/editPatchUtils";
	import { MODE_SERVICE } from "$lib/mode/modeService";
	import { getFileListContext } from "$lib/selection/fileListController.svelte";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { injectOptional, inject } from "@gitbutler/core/context";
	import { AsyncButton, FileListItem, TestId } from "@gitbutler/ui";
	import type { ConflictEntriesObj } from "$lib/files/conflicts";

	type Props = {
		projectId: string;
		stackId?: string;
		conflictEntries?: ConflictEntriesObj;
		ancestorMostConflictedCommitId?: string;
		draggable?: boolean;
	};

	const { projectId, stackId, conflictEntries, ancestorMostConflictedCommitId, draggable }: Props =
		$props();

	const controller = getFileListContext();
	const modeService = injectOptional(MODE_SERVICE, undefined);
	const userSettings = inject(SETTINGS);

	let editPatchModal: EditPatchConfirmModal | undefined = $state();
	let selectedFilePath = $state("");

	const unrepresentedConflictedEntries = $derived.by(() => {
		if (!conflictEntries?.entries) return {};

		const changes = controller.changes;
		return Object.fromEntries(
			Object.entries(conflictEntries.entries).filter(([key, _value]) =>
				changes.every((change) => change.path !== key),
			),
		);
	});

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
			projectId,
		});
	}

	function handleCancelEditPatch() {
		editPatchModal?.hide();
		selectedFilePath = "";
	}
</script>

{#if Object.keys(unrepresentedConflictedEntries).length > 0}
	{@const entries = Object.entries(unrepresentedConflictedEntries)}
	<div class="conflicted-entries">
		{#each entries as [path, kind], i}
			<FileListItem
				{draggable}
				filePath={path}
				pathFirst={$userSettings.pathFirst}
				active={controller.active}
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
							projectId,
						})}
				>
					Resolve conflicts
				</AsyncButton>
			</div>
		{/if}
	</div>
{/if}

<EditPatchConfirmModal
	bind:this={editPatchModal}
	fileName={selectedFilePath}
	onConfirm={handleConfirmEditPatch}
	onCancel={handleCancelEditPatch}
/>

<style lang="postcss">
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

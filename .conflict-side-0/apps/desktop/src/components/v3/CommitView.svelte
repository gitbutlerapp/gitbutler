<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import CommitContextMenu from '$components/v3/CommitContextMenu.svelte';
	import CommitDetails from '$components/v3/CommitDetails.svelte';
	import CommitHeader from '$components/v3/CommitHeader.svelte';
	import CommitLine from '$components/v3/CommitLine.svelte';
	import CommitMessageInput from '$components/v3/CommitMessageInput.svelte';
	import ConflictResolutionConfirmModal from '$components/v3/ConflictResolutionConfirmModal.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import { getCommitType } from '$components/v3/lib';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { type Commit } from '$lib/branches/v3';
	import { FocusManager } from '$lib/focus/focusManager.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { ModeService } from '$lib/mode/modeService';
	import { showToast } from '$lib/notifications/toasts';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { getContext, maybeGetContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import type { CommitKey } from '$lib/commits/commit';

	type Props = {
		projectId: string;
		stackId: string;
		commitKey: CommitKey;
	};

	const { projectId, stackId, commitKey }: Props = $props();

	const [stackService, uiState, focus] = inject(StackService, UiState, FocusManager);

	let conflictResolutionConfirmationModal =
		$state<ReturnType<typeof ConflictResolutionConfirmModal>>();

	const forge = getContext(DefaultForgeFactory);
	const modeService = maybeGetContext(ModeService);
	const baseBranch = getContext(BaseBranch);
	const stackState = $derived(uiState.stack(stackId));
	const selected = $derived(stackState.selection.get());
	const branchName = $derived(selected.current?.branchName);

	const commitResult = $derived(
		commitKey.upstream
			? stackService.upstreamCommitById(projectId, commitKey)
			: stackService.commitById(projectId, commitKey)
	);
	const conflicted = $derived((commitResult.current.data as Commit).hasConflicts);
	const isAncestorMostConflicted = false; // TODO
	const isUnapplied = false; // TODO
	const branchRefName = undefined; // TODO

	const [updateCommitMessage, messageUpdateResult] = stackService.updateCommitMessage;

	const focusedArea = $derived(focus.current);
	$effect(() => {
		if (focusedArea === 'commit') {
			stackState.activeSelectionId.set({ type: 'commit', commitId: commitKey.commitId });
		}
	});

	type Mode = 'view' | 'edit';

	let mode = $state<Mode>('view');
	let commitMessageInput = $state<ReturnType<typeof CommitMessageInput>>();

	function setMode(newMode: Mode) {
		mode = newMode;
	}

	async function editCommitMessage() {
		if (!branchName) {
			throw new Error('No branch selected!');
		}
		if (!commitMessageInput) return;
		const commitMessage = commitMessageInput.getMessage();
		if (!commitMessage) {
			showToast({ message: 'Commit message is required', style: 'error' });
			return;
		}

		const result = await updateCommitMessage({
			projectId,
			stackId,
			commitId: commitKey.commitId,
			message: commitMessage
		});

		if (!result.data) {
			showToast({
				message: `Update commit error`,
				style: 'error'
			});
			return;
		}

		const newCommitId = result.data;

		uiState.stack(stackId).selection.set({ branchName, commitId: newCommitId });
		setMode('view');
	}

	function getCommitTitile(message: string): string | undefined {
		// Return undefined if there is no title
		return message.split('\n').slice(0, 1).join('\n') || undefined;
	}

	function getCommitDescription(message: string): string | undefined {
		// Return undefined if there is no description
		const lines = message.split('\n');
		for (let i = 1; i < lines.length; i++) {
			if (lines[i]!.trim()) {
				return lines.slice(i).join('\n');
			}
		}
		return undefined;
	}

	function getCommitLabel(commit: Partial<Commit>) {
		const commitType = commit ? getCommitType(commit as Commit) : 'unknown';

		switch (commitType) {
			case 'local':
				return 'Unpushed';
			case 'upstream':
				return 'Upstream';
			case 'local-and-remote':
				return 'Pushed';
			case 'diverged':
				return 'Diverged';
		}
	}

	// context menu
	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabContextMenuTrigger = $state<HTMLButtonElement>();
	let isContextMenuOpen = $state(false);

	async function handleUncommit() {
		if (!baseBranch || !branchName) {
			console.error('Unable to undo commit');
			return;
		}
		await stackService.uncommit({ projectId, stackId, branchName, commitId: commitKey.commitId });
	}

	function openCommitMessageModal() {
		// TODO: Implement openCommitMessageModal
	}

	function canEdit() {
		if (isUnapplied) return false;
		if (!modeService) return false;

		return true;
	}

	async function editPatch() {
		if (!canEdit() || !branchRefName) return;
		modeService!.enterEditMode(commitKey.commitId, stackId);
	}

	async function handleEditPatch() {
		if (conflicted && !isAncestorMostConflicted) {
			conflictResolutionConfirmationModal?.show();
			return;
		}
		await editPatch();
	}
</script>

<ReduxResult {stackId} {projectId} result={commitResult.current}>
	{#snippet children(commit, env)}
		{#if mode === 'edit'}
			<Drawer
				projectId={env.projectId}
				stackId={env.stackId}
				title="Edit commit message"
				disableScroll
				minHeight={20}
			>
				<CommitMessageInput
					bind:this={commitMessageInput}
					projectId={env.projectId}
					stackId={env.stackId}
					action={editCommitMessage}
					actionLabel="Save"
					onCancel={() => setMode('view')}
					initialTitle={getCommitTitile(commit.message)}
					initialMessage={getCommitDescription(commit.message)}
					loading={messageUpdateResult.current.isLoading}
					existingCommitId={commit.id}
				/>
			</Drawer>
		{:else}
			<Drawer projectId={env.projectId} stackId={env.stackId} splitView>
				{#snippet header()}
					<div class="commit-view__header text-13">
						<CommitLine width={24} {commit} />

						<div class="commit-view__header-title text-13">
							<span class="text-semibold">{getCommitLabel(commit)} commit:</span>

							<Tooltip text="Copy commit SHA">
								<button
									type="button"
									class="commit-view__header-sha"
									onclick={() => {
										writeClipboard(commit.id, {
											message: 'Commit SHA copied'
										});
									}}
								>
									<span>
										{commit.id.substring(0, 7)}
									</span>
									<Icon name="copy-small" /></button
								>
							</Tooltip>
						</div>
					</div>
				{/snippet}

				{#snippet kebabMenu()}
					<Button
						size="tag"
						icon="kebab"
						kind="ghost"
						activated={isContextMenuOpen}
						bind:el={kebabContextMenuTrigger}
						onclick={() => {
							contextMenu?.toggle();
						}}
					/>
				{/snippet}

				<div class="commit-view">
					<CommitHeader {commit} class="text-14 text-semibold text-body" />
					<CommitDetails
						projectId={env.projectId}
						{branchName}
						{commit}
						stackId={env.stackId}
						onEditCommitMessage={() => setMode('edit')}
					/>
				</div>

				{#snippet filesSplitView()}
					<ChangedFiles
						projectId={env.projectId}
						stackId={env.stackId}
						selectionId={{ type: 'commit', commitId: commitKey.commitId }}
					/>
				{/snippet}
			</Drawer>
		{/if}

		<ConflictResolutionConfirmModal
			bind:this={conflictResolutionConfirmationModal}
			onSubmit={editPatch}
		/>

		<CommitContextMenu
			bind:menu={contextMenu}
			{projectId}
			leftClickTrigger={kebabContextMenuTrigger}
			{baseBranch}
			{stackId}
			{commit}
			commitUrl={forge.current.commitUrl(commit.id)}
			onUncommitClick={handleUncommit}
			onEditMessageClick={openCommitMessageModal}
			onPatchEditClick={handleEditPatch}
			onToggle={(isOpen) => {
				isContextMenuOpen = isOpen;
			}}
		/>
	{/snippet}
</ReduxResult>

<style>
	.commit-view {
		position: relative;
		height: 100%;
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.commit-view__header {
		display: flex;
		gap: 8px;
		height: 100%;
		margin-left: -4px;
	}

	.commit-view__header-title {
		align-self: center;
	}

	.commit-view__header-sha {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		text-decoration: dotted underline;
		transition: color var(--transition-fast);
		cursor: pointer;
		color: var(--clr-text-2);

		&:hover {
			color: var(--clr-text-1);
		}
	}
</style>

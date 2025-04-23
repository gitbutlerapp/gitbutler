<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import CommitContextMenu from '$components/v3/CommitContextMenu.svelte';
	import CommitDetails from '$components/v3/CommitDetails.svelte';
	import CommitHeader from '$components/v3/CommitHeader.svelte';
	import CommitLine from '$components/v3/CommitLine.svelte';
	import CommitMessageEditor from '$components/v3/CommitMessageEditor.svelte';
	import ConflictResolutionConfirmModal from '$components/v3/ConflictResolutionConfirmModal.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import { getCommitType, isLocalAndRemoteCommit } from '$components/v3/lib';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { isCommit, type Commit } from '$lib/branches/v3';
	import { CommitStatus, type CommitKey } from '$lib/commits/commit';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { ModeService } from '$lib/mode/modeService';
	import { showToast } from '$lib/notifications/toasts';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { inject } from '@gitbutler/shared/context';
	import { getContext, maybeGetContext } from '@gitbutler/shared/context';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	type Props = {
		projectId: string;
		stackId: string;
		commitKey: CommitKey;
	};

	const { projectId, stackId, commitKey }: Props = $props();

	const [stackService, uiState] = inject(StackService, UiState);

	let conflictResolutionConfirmationModal =
		$state<ReturnType<typeof ConflictResolutionConfirmModal>>();

	const forge = getContext(DefaultForgeFactory);
	const modeService = maybeGetContext(ModeService);
	const stackState = $derived(uiState.stack(stackId));
	const projectState = $derived(uiState.project(projectId));
	const selected = $derived(stackState.selection.get());
	const branchName = $derived(selected.current?.branchName);

	const commitResult = $derived(
		commitKey.upstream
			? stackService.upstreamCommitById(projectId, commitKey)
			: stackService.commitById(projectId, commitKey)
	);
	const isUnapplied = false; // TODO
	const branchRefName = undefined; // TODO

	const changesResult = stackService.commitChanges(projectId, commitKey.commitId);

	const [updateCommitMessage, messageUpdateResult] = stackService.updateCommitMessage;

	type Mode = 'view' | 'edit';

	let mode = $state<Mode>('view');
	let commitMessageInput = $state<ReturnType<typeof CommitMessageEditor>>();

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

		const newCommitId = await updateCommitMessage({
			projectId,
			stackId,
			commitId: commitKey.commitId,
			message: commitMessage
		});

		uiState.stack(stackId).selection.set({ branchName, commitId: newCommitId });
		setMode('view');
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
		if (!branchName) return;
		await stackService.uncommit({ projectId, stackId, branchName, commitId: commitKey.commitId });
		projectState.drawerPage.set(undefined);
		if (branchName) stackState.selection.set({ branchName, commitId: undefined });
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
		await modeService!.enterEditMode(commitKey.commitId, stackId);
	}
</script>

<ReduxResult {stackId} {projectId} result={commitResult.current}>
	{#snippet children(commit, env)}
		{@const isConflicted = isCommit(commit) && commit.hasConflicts}
		{#if mode === 'edit'}
			<Drawer
				projectId={env.projectId}
				stackId={env.stackId}
				title="Edit commit message"
				disableScroll
				minHeight={20}
			>
				<CommitMessageEditor
					bind:this={commitMessageInput}
					projectId={env.projectId}
					stackId={env.stackId}
					action={editCommitMessage}
					actionLabel="Save"
					onCancel={() => setMode('view')}
					initialTitle={splitMessage(commit.message).title}
					initialMessage={splitMessage(commit.message).description}
					loading={messageUpdateResult.current.isLoading}
					existingCommitId={commit.id}
				/>
			</Drawer>
		{:else}
			<Drawer projectId={env.projectId} stackId={env.stackId}>
				{#snippet header()}
					<div class="commit-view__header text-13">
						{#if isLocalAndRemoteCommit(commit)}
							<CommitLine
								commitStatus={commit.state.type}
								diverged={commit.state.type === 'LocalAndRemote' &&
									commit.id !== commit.state.subject}
								tooltip={commit.state.type}
								width={24}
							/>
						{:else}
							<CommitLine
								commitStatus="Remote"
								diverged={false}
								tooltip={CommitStatus.Remote}
								width={24}
							/>
						{/if}

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
					<CommitHeader
						commitMessage={commit.message}
						className="text-14 text-semibold text-body"
					/>
					<CommitDetails {commit}>
						<Button
							size="tag"
							kind="outline"
							icon="edit-small"
							onclick={() => {
								openCommitMessageModal();
							}}
						>
							Edit message
						</Button>

						{#if !isConflicted}
							<AsyncButton
								size="tag"
								kind="outline"
								icon="undo-small"
								action={async () => await handleUncommit()}
							>
								Uncommit
							</AsyncButton>
						{/if}

						<AsyncButton size="tag" kind="outline" action={editPatch}>
							{#if isConflicted}
								Resolve conflicts
							{:else}
								Edit commit
							{/if}
						</AsyncButton>
					</CommitDetails>
				</div>

				{#snippet filesSplitView()}
					<ReduxResult {projectId} {stackId} result={changesResult.current}>
						{#snippet children(changes)}
							<ChangedFiles
								title="Changed files"
								projectId={env.projectId}
								stackId={env.stackId}
								selectionId={{ type: 'commit', commitId: commit.id }}
								{changes}
							/>
						{/snippet}
					</ReduxResult>
				{/snippet}
			</Drawer>
		{/if}

		<ConflictResolutionConfirmModal
			bind:this={conflictResolutionConfirmationModal}
			onSubmit={editPatch}
		/>

		<ContextMenu leftClickTrigger={kebabContextMenuTrigger}>
			{#snippet menu({ close })}
				<CommitContextMenu
					{close}
					{projectId}
					{stackId}
					commitId={commit.id}
					commitMessage={commit.message}
					commitStatus={isLocalAndRemoteCommit(commit) ? commit.state.type : 'Remote'}
					commitUrl={forge.current.commitUrl(commit.id)}
					onUncommitClick={handleUncommit}
					onEditMessageClick={openCommitMessageModal}
					onToggle={(isOpen) => {
						isContextMenuOpen = isOpen;
					}}
				/>
			{/snippet}
		</ContextMenu>
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

<script lang="ts">
	import BranchFilesList from '$components/BranchFilesList.svelte';
	import CommitContextMenu from '$components/CommitContextMenu.svelte';
	import CommitMessageInput from '$components/CommitMessageInput.svelte';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { BranchStack } from '$lib/branches/branch';
	import { PatchSeries } from '$lib/branches/branch';
	import { Commit, DetailedCommit } from '$lib/commits/commit';
	import { type CommitStatusType } from '$lib/commits/commit';
	import { createCommitStore } from '$lib/commits/contexts';
	import { CommitDropData } from '$lib/commits/dropHandler';
	import { persistedCommitMessage } from '$lib/config/config';
	import { draggableCommit } from '$lib/dragging/draggable';
	import { NON_DRAGGABLE } from '$lib/dragging/draggables';
	import { RemoteFile } from '$lib/files/file';
	import { FileService } from '$lib/files/fileService';
	import { ModeService } from '$lib/mode/modeService';
	import { Project } from '$lib/project/project';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UserService } from '$lib/user/userService';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext, maybeGetContext } from '@gitbutler/shared/context';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import PopoverActionsContainer from '@gitbutler/ui/popoverActions/PopoverActionsContainer.svelte';
	import PopoverActionsItem from '@gitbutler/ui/popoverActions/PopoverActionsItem.svelte';
	import { marked } from '@gitbutler/ui/utils/marked';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import { type Snippet } from 'svelte';

	const userService = getContext(UserService);
	const user = userService.user;

	interface Props {
		projectId?: string;
		stack?: BranchStack | undefined;
		currentSeries?: PatchSeries | undefined;
		commit: DetailedCommit | Commit;
		commitUrl?: string | undefined;
		isHeadCommit?: boolean;
		isUnapplied?: boolean;
		last?: boolean;
		noBorder?: boolean;
		type: CommitStatusType;
		lines?: Snippet | undefined;
		filesToggleable?: boolean;
		disableCommitActions?: boolean;
	}

	const {
		projectId,
		stack = undefined,
		currentSeries,
		commit,
		commitUrl = undefined,
		isHeadCommit = false,
		isUnapplied = false,
		last = false,
		noBorder = false,
		type,
		lines = undefined,
		filesToggleable = true,
		disableCommitActions = false
	}: Props = $props();

	const baseBranch = getContext(BaseBranch);
	const project = getContext(Project);
	const modeService = maybeGetContext(ModeService);
	const fileService = getContext(FileService);
	const stackService = getContext(StackService);

	const [updateCommitMessage] = stackService.updateCommitMessage;

	const commitStore = createCommitStore(commit);

	$effect(() => {
		commitStore.set(commit);
	});

	const persistedMessage = persistedCommitMessage(project.id, stack?.id || '');

	let branchCardElement = $state<HTMLElement>();
	let kebabMenuTrigger = $state<HTMLButtonElement>();
	let draggableCommitElement = $state<HTMLElement>();
	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let isOpenedByKebabButton = $state(false);
	let isOpenByMouse = $state(false);

	let files = $state<RemoteFile[]>([]);
	let showDetails = $state(false);
	let conflictResolutionConfirmationModal = $state<ReturnType<typeof Modal>>();

	const conflicted = $derived(commit.conflicted);
	const isAncestorMostConflicted = $derived(
		currentSeries?.ancestorMostConflictedCommit?.id === commit.id
	);

	async function loadFiles() {
		files = await fileService.listCommitFiles(project.id, commit.id);
	}

	function toggleFiles() {
		if (!filesToggleable) return;
		showDetails = !showDetails;

		if (showDetails) loadFiles();
	}

	function onKeyup(e: KeyboardEvent) {
		if (e.key === 'Enter' || e.key === ' ') {
			toggleFiles();
		}
	}

	async function undoCommit() {
		if (!stack || !baseBranch || !currentSeries) {
			console.error('Unable to undo commit');
			return;
		}
		$persistedMessage = commit.description;
		description = commit.description;
		await stackService.uncommit({
			projectId: project.id,
			stackId: stack.id,
			branchName: currentSeries.name,
			commitId: commit.id
		});
	}

	let isUndoable = commit instanceof DetailedCommit && type !== 'Remote' && type !== 'Integrated';

	let commitMessageModal: ReturnType<typeof Modal> | undefined;
	let commitMessageValid = $state(false);
	let description = $state('');

	function openCommitMessageModal(e: MouseEvent) {
		e.stopPropagation();
		description = commit.description;
		commitMessageModal?.show();
	}

	let isUpdating = $state(false);

	async function submitCommitMessageModal() {
		if (!stack) {
			return;
		}
		isUpdating = true;
		commit.description = description;
		await updateCommitMessage({
			projectId: project.id,
			stackId: stack.id,
			commitId: commit.id,
			message: description
		});

		commitMessageModal?.close();
		isUpdating = false;
	}

	const commitShortSha = commit.id.substring(0, 7);
	const authorImgUrl = $derived.by(() => {
		return commit.author.email?.toLowerCase() === $user?.email?.toLowerCase()
			? $user?.picture
			: commit.author.gravatarUrl;
	});

	function canEdit() {
		if (isUnapplied) return false;
		if (!modeService) return false;
		if (!stack) return false;

		return true;
	}

	async function editPatch() {
		if (!canEdit()) return;
		await modeService!.enterEditMode(commit.id, stack!.id);
	}

	async function handleEditPatch() {
		if (conflicted && !isAncestorMostConflicted) {
			conflictResolutionConfirmationModal?.show();
			return;
		}
		await editPatch();
	}

	const showOpenInBrowser = $derived(commitUrl && (type === 'Remote' || type === 'LocalAndRemote'));
	const isDraggable = commit instanceof DetailedCommit && !isUnapplied && type !== 'Integrated';
</script>

<Modal bind:this={commitMessageModal} width="small" onSubmit={submitCommitMessageModal}>
	{#snippet children(_, close)}
		<CommitMessageInput
			bind:commitMessage={description}
			bind:valid={commitMessageValid}
			existingCommit={commit}
			isExpanded={true}
			cancel={close}
			commit={submitCommitMessageModal}
		/>
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="neutral" type="submit" grow disabled={!commitMessageValid} loading={isUpdating}
			>Submit</Button
		>
	{/snippet}
</Modal>

<Modal bind:this={conflictResolutionConfirmationModal} width="small">
	{#snippet children()}
		<div>
			<p>It's generally better to start resolving conflicts from the bottom up.</p>
			<br />
			<p>Are you sure you want to resolve conflicts for this commit?</p>
		</div>
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<AsyncButton
			style="pop"
			action={async () => {
				await editPatch();
				close();
			}}>Yes</AsyncButton
		>
	{/snippet}
</Modal>

<CommitContextMenu
	leftClickTrigger={kebabMenuTrigger}
	rightClickTrigger={branchCardElement}
	onToggle={(isOpen, isLeftClick) => {
		if (isLeftClick) {
			isOpenedByKebabButton = isOpen;
		} else {
			isOpenByMouse = isOpen;
		}
	}}
	bind:menu={contextMenu}
	{baseBranch}
	{stack}
	{commit}
	isRemote={type === 'Remote'}
	commitUrl={showOpenInBrowser ? commitUrl : undefined}
	onUncommitClick={undoCommit}
	onEditMessageClick={openCommitMessageModal}
/>

<div
	bind:this={branchCardElement}
	class="commit-row"
	class:is-commit-open={showDetails}
	class:not-draggable={!isDraggable}
	class:commit-card-activated={isOpenedByKebabButton || isOpenByMouse}
	class:is-last={last}
	class:no-border={last || noBorder}
	onclick={(e) => {
		e.preventDefault();
		toggleFiles();
	}}
	oncontextmenu={(e) => {
		e.preventDefault();
		isOpenedByKebabButton = false;
		contextMenu?.open(e);
	}}
	onkeyup={onKeyup}
	role="button"
	tabindex="0"
	use:draggableCommit={isDraggable && stack
		? {
				disabled: false,
				label: commit.descriptionTitle,
				sha: commitShortSha,
				date: getTimeAgo(commit.createdAt),
				authorImgUrl: authorImgUrl,
				commitType: type,
				data: new CommitDropData(
					stack.id,
					{
						id: commit.id,
						hasConflicts: commit.conflicted,
						isRemote: commit instanceof Commit,
						isIntegrated: commit instanceof DetailedCommit && commit.isIntegrated
					},
					isHeadCommit,
					currentSeries?.name
				),
				viewportId: 'board-viewport'
			}
		: NON_DRAGGABLE}
>
	{#if lines}
		<div>
			{@render lines()}
		</div>
	{/if}

	{#if !disableCommitActions}
		<PopoverActionsContainer class="commit-actions-menu" thin stayOpen={isOpenedByKebabButton}>
			<PopoverActionsItem
				bind:el={kebabMenuTrigger}
				activated={isOpenedByKebabButton}
				icon="kebab"
				tooltip="More options"
				thin
				onclick={() => {
					contextMenu?.toggle();
				}}
			/>
		</PopoverActionsContainer>
	{/if}

	<div class="commit-card" class:is-last={last} class:no-border={last || noBorder}>
		<!-- GENERAL INFO -->
		<div bind:this={draggableCommitElement} class="commit__header" role="button" tabindex="-1">
			{#if !disableCommitActions}
				<div class="commit__drag-icon">
					<Icon name="draggable" />
				</div>
			{/if}

			{#if isUndoable && !commit.descriptionTitle}
				<span class="text-13 text-body text-semibold commit__empty-title">empty commit message</span
				>
			{:else}
				<h5 class="text-13 text-body text-semibold commit__title" class:truncate={!showDetails}>
					{commit.descriptionTitle}
				</h5>

				<div class="text-11 text-semibold commit__subtitle">
					{#if commit.isSigned}
						<Tooltip text="Signed">
							<div class="commit__signed">
								<Icon name="success-outline-small" />
							</div>
						</Tooltip>

						<span class="commit__subtitle-divider">•</span>
					{/if}

					{#if conflicted}
						<Tooltip
							text={"Conflicted commits must be resolved before they can be amended or squashed.\nPlease resolve conflicts using the 'Resolve conflicts' button"}
						>
							<div class="commit__conflicted">
								<Icon name="warning-small" />

								<span>Conflicted</span>
							</div>
						</Tooltip>

						<span class="commit__subtitle-divider">•</span>
					{/if}

					<Tooltip text={commit.author.name}>
						<img class="commit__subtitle-avatar" src={authorImgUrl} alt="" />
					</Tooltip>

					<span class="commit__subtitle-divider">•</span>

					<button
						type="button"
						class="commit__subtitle-btn commit__subtitle-btn_dashed"
						onclick={(e) => {
							e.stopPropagation();
							writeClipboard(commit.id);
						}}
					>
						<span>{commitShortSha}</span>

						<div class="commit__subtitle-btn__icon">
							<Icon name="copy-small" />
						</div>
					</button>

					{#if showDetails && showOpenInBrowser}
						<span class="commit__subtitle-divider">•</span>

						<button
							type="button"
							class="commit__subtitle-btn"
							onclick={(e) => {
								e.stopPropagation();
								if (commitUrl) openExternalUrl(commitUrl);
							}}
						>
							<span>Open</span>

							<div class="commit__subtitle-btn__icon">
								<Icon name="open-link" />
							</div>
						</button>
					{/if}
					<span class="commit__subtitle-divider">•</span>
					<span>{getTimeAgo(commit.createdAt)}</span>
				</div>
			{/if}
		</div>

		<!-- HIDDEN -->
		{#if showDetails}
			{#if commit.descriptionBody || isUndoable}
				<div class="commit__details">
					{#if commit.descriptionBody}
						<span class="commit__description text-12 text-body">
							{@html marked(commit.descriptionBody)}
						</span>
					{/if}

					{#if isUndoable}
						<div class="commit__actions hide-native-scrollbar">
							{#if isUndoable}
								{#if !conflicted}
									<Button size="tag" kind="outline" icon="undo-small" onclick={() => undoCommit()}>
										Uncommit
									</Button>
								{/if}
								<Button
									size="tag"
									kind="outline"
									icon="edit-small"
									onclick={(e: MouseEvent) => {
										openCommitMessageModal(e);
									}}>Edit message</Button
								>
							{/if}
							{#if canEdit()}
								<AsyncButton size="tag" kind="outline" action={handleEditPatch} stopPropagation>
									{#if conflicted}
										Resolve conflicts
									{:else}
										Edit commit
									{/if}
								</AsyncButton>
							{/if}
						</div>
					{/if}
				</div>
			{/if}

			<div class="files-container">
				<BranchFilesList
					{projectId}
					allowMultiple={!isUnapplied && type !== 'Remote'}
					{files}
					{isUnapplied}
					conflictedFiles={commit.conflictedFiles}
					readonly={type === 'Remote' || isUnapplied}
				/>
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.commit-row {
		position: relative;
		display: flex;
		gap: 12px;
		width: 100%;
		background-color: var(--clr-bg-1);
		transition: background-color var(--transition-fast);

		&:focus {
			outline: none;
		}

		&:hover {
			& :global(.commit-actions-menu) {
				--show: true;
			}
		}

		&:not(.is-commit-open) {
			&:hover {
				background-color: var(--clr-bg-1-muted);

				& .commit__drag-icon {
					opacity: 1;
				}
			}
		}

		&.not-draggable {
			&:hover {
				& .commit__drag-icon {
					pointer-events: none;
					opacity: 0;
				}
			}
		}

		&:not(.no-border) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&.is-last {
			border-radius: 0 0 var(--radius-m) var(--radius-m);
		}
	}

	.commit-card-activated {
		background-color: var(--clr-bg-1-muted);
	}

	.commit-card {
		display: flex;
		position: relative;
		flex-direction: column;
		flex: 1;
		overflow: hidden;
	}

	.commit__conflicted {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--clr-core-err-40);
	}

	/* HEADER */
	.commit__header {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 14px 14px 14px 0;
	}

	.commit__drag-icon {
		cursor: grab;
		position: absolute;
		display: flex;
		bottom: 10px;
		right: 6px;
		color: var(--clr-text-3);

		opacity: 0;
		transition: opacity var(--transition-fast);
	}

	.commit__title {
		flex: 1;
		text-align: left;
		display: block;
		color: var(--clr-text-1);
		width: 100%;
	}

	.commit__description {
		color: var(--clr-text-2);
		white-space: pre-wrap;
		user-select: text;
		cursor: text;
	}

	.commit__empty-title {
		color: var(--clr-text-3);
	}

	.commit__subtitle {
		display: flex;
		align-items: center;
		flex-wrap: nowrap;
		gap: 4px;
		color: var(--clr-text-2);
		overflow: hidden;

		& > span {
			white-space: nowrap;
			overflow: hidden;
			text-overflow: ellipsis;
		}
	}

	.commit__signed {
		display: flex;
	}

	/* SUBTITLE LINK BUTTON */
	.commit__subtitle-btn {
		flex-shrink: 0;
		display: flex;
		align-items: center;

		text-decoration-line: underline;
		text-underline-offset: 2px;
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-1);

			& .commit__subtitle-btn__icon {
				width: var(--size-icon);
				opacity: 1;
				margin-left: 2px;
				transform: scale3d(1, 1, 1);
			}
		}
	}

	.commit__subtitle-btn_dashed {
		text-decoration-style: dashed;
	}

	.commit__subtitle-btn__icon {
		display: flex;
		width: 0;
		opacity: 0;
		margin-left: 0;
		transform: scale3d(0.6, 0.6, 0.6); /* CSS glitch fix */
		transition:
			width var(--transition-medium),
			opacity var(--transition-fast),
			color var(--transition-fast),
			transform var(--transition-medium),
			margin var(--transition-fast);
	}

	.commit__subtitle-avatar {
		width: 12px;
		height: 12px;
		border-radius: 50%;
	}

	/* DIVIDER - DOT SYMBOL */
	.commit__subtitle-divider {
		opacity: 0.4;
	}

	/* DETAILS */
	.commit__details {
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding-bottom: 12px;
		padding-right: 14px;
	}

	.commit__actions {
		overflow: visible;
		display: flex;
		gap: 4px;
		overflow-x: auto;
	}

	/* FILES */
	.files-container {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		margin-right: 14px;
		margin-bottom: 14px;
		overflow: hidden;
	}

	/* MODIFIERS */
	.is-commit-open {
		& .commit__subtitle-btn__icon {
			width: var(--size-icon);
			opacity: 1;
			margin-left: 2px;
			transform: scale3d(1, 1, 1);
		}
	}
</style>

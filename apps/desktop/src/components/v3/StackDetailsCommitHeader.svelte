<script lang="ts">
	import CommitMessageInput from '$components/v3/CommitMessageInput.svelte';
	import { ModeService } from '$lib/mode/modeService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	// import { openExternalUrl } from '$lib/utils/url';
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { Commit, WorkspaceBranch } from '$lib/branches/v3';

	interface Props {
		commit: Commit;
		stackId: string;
		projectId: string;
		selectedBranchDetails?: WorkspaceBranch;
	}

	const { commit, stackId, projectId, selectedBranchDetails }: Props = $props();

	const [stackService, modeService] = inject(StackService, ModeService);
	const commitShortSha = $derived(commit.id.substring(0, 7));
	const commitMessage = $derived(commit.message.trim());

	let conflictResolutionConfirmationModal: ReturnType<typeof Modal> | undefined;
	let commitMessageModal: ReturnType<typeof Modal> | undefined;
	let commitMessageValid = $state(false);
	let message = $state(commit.message);

	let isUndoable = commit.state.type !== 'Integrated';

	function submitCommitMessageModal() {
		if (stackId) {
			stackService.updateCommitMessage(projectId, stackId, commit.id, message);
		}

		commitMessageModal?.close();
	}

	function openCommitMessageModal(e: MouseEvent) {
		e.stopPropagation();
		commitMessageModal?.show();
	}

	function handleUncommit(e: MouseEvent) {
		e.stopPropagation();
		// TODO: Wire up uncommit btn.
		// currentCommitMessage.set(commit.description);
		// undoCommit(commit);
	}

	async function editPatch() {
		if (selectedBranchDetails?.name) {
			modeService!.enterEditMode(commit.id, `refs/heads/${selectedBranchDetails.name}`);
		}
	}

	async function handleEditPatch() {
		if (!commit.hasConflicts) {
			conflictResolutionConfirmationModal?.show();
			return;
		}
		await editPatch();
	}
</script>

<div class="wrapper">
	<div class="message text-12">{commitMessage}</div>
	<div class="metadata text-11 text-semibold">
		{#if commit.hasConflicts}
			<Tooltip
				text={"Conflicted commits must be resolved before they can be amended or squashed.\nPlease resolve conflicts using the 'Resolve conflicts' button"}
			>
				<span class="conflicted text-13">Conflicted</span>
			</Tooltip>

			<span class="divider">•</span>
		{/if}
		<Tooltip text={commit.author.name}>
			<img class="avatar" src={commit.author.gravatarUrl} alt={`${commit.author.name} Avatar`} />
		</Tooltip>
		<span class="divider">•</span>
		<button
			type="button"
			class="commit-sha-btn"
			onclick={(e) => {
				e.stopPropagation();
				copyToClipboard(commit.id);
			}}
		>
			<span class="">{commitShortSha}</span>
			<div class="">
				<Icon name="copy-small" />
			</div>
		</button>
		<span class="divider">•</span>
		<button
			type="button"
			class="open-external-btn"
			onclick={(e) => {
				e.stopPropagation();
				// TODO: Generate commitUrl.
				// if (commitUrl) openExternalUrl(commitUrl);
			}}
		>
			<span>Open</span>

			<div class="">
				<Icon name="open-link" />
			</div>
		</button>
		<span class="divider">•</span>
		<span class="">{getTimeAgo(new Date(commit.createdAt))}</span>
	</div>
	<div class="actions">
		{#if isUndoable}
			<Button
				size="tag"
				kind="outline"
				icon="edit-small"
				onclick={(e: MouseEvent) => {
					openCommitMessageModal(e);
				}}>Edit message</Button
			>
			{#if !commit.hasConflicts}
				<Button
					size="tag"
					kind="outline"
					icon="undo-small"
					onclick={(e: MouseEvent) => {
						handleUncommit(e);
					}}>Undo</Button
				>
			{/if}
		{/if}
		<Button size="tag" kind="outline" onclick={handleEditPatch}>
			{#if commit.hasConflicts}
				Resolve conflicts
			{:else}
				Edit commit
			{/if}
		</Button>
	</div>
</div>

<Modal bind:this={commitMessageModal} width="small" onSubmit={submitCommitMessageModal}>
	{#snippet children(_, close)}
		<CommitMessageInput
			bind:commitMessage={message}
			bind:valid={commitMessageValid}
			branchName={selectedBranchDetails?.name}
			existingCommit={commit}
			isExpanded={true}
			cancel={close}
			commit={submitCommitMessageModal}
		/>
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="neutral" type="submit" grow disabled={!commitMessageValid}>Submit</Button>
	{/snippet}
</Modal>

<Modal bind:this={conflictResolutionConfirmationModal} width="small" onSubmit={editPatch}>
	{#snippet children()}
		<div>
			<p>It's generally better to start resolving conflicts from the bottom up.</p>
			<br />
			<p>Are you sure you want to resolve conflicts for this commit?</p>
		</div>
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit">Yes</Button>
	{/snippet}
</Modal>

<style>
	.wrapper {
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--clr-border-2);
		gap: 8px;
		padding: 14px 14px 16px 14px;
	}

	.message {
		white-space: pre-wrap;
		line-height: 18px;

		&::first-line {
			font-size: 16px;
			font-weight: 500;
		}
	}

	.metadata {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--clr-text-2);

		& .divider {
			font-size: 12px;
			opacity: 0.4;
		}
	}

	.actions {
		display: flex;
		gap: 4px;
		margin-top: 8px;
	}

	.avatar {
		align-self: center;
		border-radius: 50%;
		width: 16px;
		aspect-ratio: 1/1;
	}

	.commit-sha-btn {
		display: flex;
		align-items: center;
		gap: 2px;

		/* TODO: `underline dashed` broken on Linux */
		text-decoration-line: underline;
		text-underline-offset: 2px;
		text-decoration-style: dashed;
	}

	.open-external-btn {
		display: flex;
		align-items: center;
		gap: 2px;
	}

	.conflicted {
		color: var(--clr-theme-err-element);
	}
</style>

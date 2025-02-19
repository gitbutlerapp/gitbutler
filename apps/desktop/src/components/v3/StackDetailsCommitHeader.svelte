<script lang="ts">
	import CommitMessageInput from '$components/v3/CommitMessageInput.svelte';
	import { Commit } from '$lib/commits/commit';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';

	interface Props {
		commit: Commit;
		stackId: string;
		projectId: string;
	}

	const { commit, stackId, projectId }: Props = $props();

	const [stackService] = inject(StackService);
	const commitShortSha = $derived(commit.id.substring(0, 7));

	let conflictResolutionConfirmationModal: ReturnType<typeof Modal> | undefined;
	let commitMessageModal: ReturnType<typeof Modal> | undefined;
	let commitMessageValid = $state(false);
	let description = $state('');

	// let isUndoable = commit instanceof DetailedCommit && type !== 'remote' && type !== 'integrated';
	let isUndoable = true;

	function submitCommitMessageModal() {
		commit.description = description;

		if (stackId) {
			stackService.updateCommitMessage(projectId, stackId, commit.id, description);
		}

		commitMessageModal?.close();
	}

	function openCommitMessageModal(e: MouseEvent) {
		e.stopPropagation();
		description = commit.description;
		commitMessageModal?.show();
	}

	function handleUncommit(e: MouseEvent) {
		e.stopPropagation();
		// currentCommitMessage.set(commit.description);
		// undoCommit(commit);
	}

	function canEdit() {
		// if (isUnapplied) return false;
		// if (!modeService) return false;
		// if (!branch) return false;

		return true;
	}

	async function editPatch() {
		if (!canEdit()) return;
		// modeService!.enterEditMode(commit.id, branch!.refname);
	}

	async function handleEditPatch() {
		// if (conflicted && !isAncestorMostConflicted) {
		// 	conflictResolutionConfirmationModal?.show();
		// 	return;
		// }
		await editPatch();
	}
</script>

<div class="wrapper">
	<div class="message text-12">{commit.description}</div>
	<div class="metadata text-11 text-semibold">
		{#if commit.conflicted}
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
			{#if !commit.conflicted}
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
		{#if canEdit()}
			<Button size="tag" kind="outline" onclick={handleEditPatch}>
				{#if commit.conflicted}
					Resolve conflicts
				{:else}
					Edit commit
				{/if}
			</Button>
		{/if}
	</div>
</div>

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

		/* `underline dashed` broken on Linux */
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

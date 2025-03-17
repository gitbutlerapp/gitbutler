<script lang="ts">
	import CommitMessageInput from '$components/v3/CommitMessageInput.svelte';
	import { ModeService } from '$lib/mode/modeService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	// import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import type { Commit } from '$lib/branches/v3';
	import type { CommitKey } from '$lib/commits/commit';

	interface Props {
		projectId: string;
		commitKey: CommitKey;
		commit: Commit;
		href?: string;
		onclick?: () => void;
	}

	const { projectId, commitKey, commit }: Props = $props();
	const { stackId, branchName } = $derived(commitKey);

	const [stackService, modeService] = inject(StackService, ModeService);

	let conflictResolutionConfirmationModal: ReturnType<typeof Modal> | undefined;
	let commitMessageModal: ReturnType<typeof Modal> | undefined;
	let commitMessageValid = $state(false);
	const message = $derived(commit.message);

	let isUndoable = $derived(commit.state.type !== 'Integrated');

	function submitCommitMessageModal() {
		stackService.updateCommitMessage(projectId, stackId, commit.id, message);

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
		if (branchName) {
			modeService!.enterEditMode(commit.id, stackId);
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

<div class="actions">
	{#if commit.hasConflicts}
		<Tooltip
			text={"Conflicted commits must be resolved before they can be amended or squashed.\nPlease resolve conflicts using the 'Resolve conflicts' button"}
		>
			<span class="conflicted text-13">Conflicted</span>
		</Tooltip>
	{/if}
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

<Modal bind:this={commitMessageModal} width="small" onSubmit={submitCommitMessageModal}>
	{#snippet children(_, close)}
		<CommitMessageInput
			bind:commitMessage={commit.message}
			bind:valid={commitMessageValid}
			{branchName}
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
	.actions {
		display: flex;
		gap: 4px;
		margin-top: 8px;
	}

	.conflicted {
		color: var(--clr-theme-err-element);
	}
</style>

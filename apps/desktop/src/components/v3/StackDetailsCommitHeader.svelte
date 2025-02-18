<script lang="ts">
	import { Commit } from '$lib/commits/commit';
	import { openExternalUrl } from '$lib/utils/url';
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';

	interface Props {
		commit: Commit;
	}

	const { commit }: Props = $props();

	const commitShortSha = $derived(commit.id.substring(0, 7));

	// let isUndoable = commit instanceof DetailedCommit && type !== 'remote' && type !== 'integrated';
	let isUndoable = true;

	function openCommitMessageModal(e: MouseEvent) {
		e.stopPropagation();
		// description = commit.description;
		// commitMessageModal?.show();
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
	<div class="title text-16 text-bold">{commit.description}</div>
	<div class="description text-12">{commit.description}</div>
	<div class="metadata text-11 text-semibold">
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

<style>
	.wrapper {
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--clr-border-2);
		gap: 8px;
		padding: 14px 14px 16px 14px;
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
		text-decoration-line: underline;
		text-underline-offset: 2px;
		text-decoration-style: dashed;
	}

	.open-external-btn {
		display: flex;
		align-items: center;
		gap: 2px;
	}
</style>

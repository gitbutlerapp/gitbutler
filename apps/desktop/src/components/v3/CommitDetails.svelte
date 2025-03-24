<script lang="ts">
	import { isCommit, type Commit, type UpstreamCommit } from '$lib/branches/v3';
	import { ModeService } from '$lib/mode/modeService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UserService } from '$lib/user/userService';
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import { marked } from '@gitbutler/ui/utils/marked';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';

	type Props = {
		projectId: string;
		stackId: string;
		commit: UpstreamCommit | Commit;
		href?: string;
		onclick?: () => void;
		onEditCommitMessage: () => void;
	};

	const { projectId, commit, onclick, stackId, onEditCommitMessage }: Props = $props();

	const [userService, modeService, stackService] = inject(UserService, ModeService, StackService);

	const [uncommit, uncommitResult] = stackService.uncommit();
	const user = $derived(userService.user);

	const commitShortSha = $derived(commit.id.substring(0, 7));

	const message = $derived(commit.message);
	const description = $derived(message.slice(message.indexOf('\n') + 1).trim());
	const isConflicted = $derived(isCommit(commit) && commit.hasConflicts);
	const isUpstream = $derived(!isCommit(commit));

	function getGravatarUrl(email: string, existingGravatarUrl: string): string {
		if ($user?.email === undefined) {
			return existingGravatarUrl;
		}
		if (email === $user.email) {
			return $user.picture ?? existingGravatarUrl;
		}
		return existingGravatarUrl;
	}

	async function editPatch() {
		await modeService.enterEditMode(commit.id, stackId);
	}

	async function handleEditPatch() {
		await editPatch();
	}

	async function handleUncommit(e: MouseEvent) {
		e.stopPropagation();
		await uncommit({ projectId, stackId, commitId: commit.id });
	}

	function openCommitMessageModal() {
		onEditCommitMessage();
	}
</script>

<div class="commit-header" role="button" {onclick} onkeypress={onclick} tabindex="0">
	<div class="metadata text-11 text-semibold">
		<span>Author:</span>
		<Avatar
			size={'medium'}
			tooltip={commit.author.name}
			srcUrl={getGravatarUrl(commit.author.email, commit.author.gravatarUrl)}
		/>
		<span class="divider">•</span>
		<button
			type="button"
			class="commit-sha-btn"
			onclick={(e) => {
				e.stopPropagation();
				copyToClipboard(commit.id);
			}}
		>
			<span>{commitShortSha}</span>
			<Icon name="copy-small" />
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

			<div>
				<Icon name="open-link" />
			</div>
		</button>
		<span class="divider">•</span>
		<span>{getTimeAgo(new Date(commit.createdAt))}</span>
	</div>

	<div class="commit-details_actions">
		{#if !isUpstream}
			<Button
				size="tag"
				kind="outline"
				icon="edit-small"
				onclick={() => {
					openCommitMessageModal();
				}}>Edit message</Button
			>

			{#if !isConflicted}
				<Button
					size="tag"
					kind="outline"
					icon="undo-small"
					loading={uncommitResult.current.isLoading}
					onclick={(e: MouseEvent) => {
						handleUncommit(e);
					}}>Uncommit</Button
				>
			{/if}

			<Button size="tag" kind="outline" onclick={handleEditPatch}>
				{#if isConflicted}
					Resolve conflicts
				{:else}
					Edit commit
				{/if}
			</Button>
		{/if}
	</div>

	{#if description}
		<div class="text-13 commit-description">
			{@html marked(description)}
		</div>
	{/if}
</div>

<style>
	.commit-header {
		display: flex;
		flex-direction: column;
		gap: 16px;
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

	.commit-description {
		padding-bottom: 8px;
	}

	.commit-details_actions {
		width: 100%;
		display: flex;
		gap: 5px;
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
</style>

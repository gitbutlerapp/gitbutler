<script lang="ts">
	import { isCommit, type Commit, type UpstreamCommit } from '$lib/branches/v3';
	import { ModeService } from '$lib/mode/modeService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UserService } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import { marked } from '@gitbutler/ui/utils/marked';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';

	type Props = {
		projectId: string;
		stackId: string;
		commit: UpstreamCommit | Commit;
		href?: string;
		onEditCommitMessage: () => void;
	};

	const { projectId, commit, stackId, onEditCommitMessage }: Props = $props();

	const [userService, modeService, stackService] = inject(UserService, ModeService, StackService);

	const [uncommit, uncommitResult] = stackService.uncommit;
	const user = $derived(userService.user);

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

<div class="commit-header">
	<div class="metadata text-12">
		<span>Author:</span>
		<Avatar
			size={'medium'}
			tooltip={commit.author.name}
			srcUrl={getGravatarUrl(commit.author.email, commit.author.gravatarUrl)}
		/>
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
		<p class="text-13 text-body commit-description">
			{@html marked(description)}
		</p>
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
</style>

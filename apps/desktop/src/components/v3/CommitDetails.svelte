<script lang="ts">
	import { isCommit, type Commit, type UpstreamCommit } from '$lib/branches/v3';
	import { ModeService } from '$lib/mode/modeService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { UserService } from '$lib/user/userService';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { inject } from '@gitbutler/shared/context';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import { marked } from '@gitbutler/ui/utils/marked';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';

	type Props = {
		projectId: string;
		stackId: string;
		commit: UpstreamCommit | Commit;
		branchName?: string;
		href?: string;
		onEditCommitMessage: () => void;
	};

	const { projectId, commit, stackId, onEditCommitMessage }: Props = $props();

	const [userService, modeService, stackService, uiState] = inject(
		UserService,
		ModeService,
		StackService,
		UiState
	);

	const user = $derived(userService.user);
	const stackState = $derived(uiState.stack(stackId));
	const projectState = $derived(uiState.project(projectId));
	const selected = $derived(stackState.selection.get());
	const branchName = $derived(selected.current?.branchName);

	const message = $derived(commit.message);
	const description = $derived(splitMessage(message).description);
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

	async function handleUncommit() {
		if (!branchName) return;
		await stackService.uncommit({ projectId, stackId, branchName, commitId: commit.id });
		projectState.drawerPage.set(undefined);
		if (branchName) stackState.selection.set({ branchName, commitId: undefined });
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

	{#if !isUpstream}
		<div class="commit-details_actions">
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

			<AsyncButton size="tag" kind="outline" action={handleEditPatch}>
				{#if isConflicted}
					Resolve conflicts
				{:else}
					Edit commit
				{/if}
			</AsyncButton>
		</div>
	{/if}

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

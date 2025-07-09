<script lang="ts">
	import { writeClipboard } from '$lib/backend/clipboard';
	import { isCommit, type Commit, type UpstreamCommit } from '$lib/branches/v3';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { UserService } from '$lib/user/userService';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { Snippet } from 'svelte';

	type Props = {
		commit: UpstreamCommit | Commit;
		children?: Snippet;
	};

	const { commit, children }: Props = $props();

	const [userService] = inject(UserService, UiState);

	const user = $derived(userService.user);

	const message = $derived(commit.message);
	const { description } = $derived(splitMessage(message));
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

	let showFullDescription = $state(false);

	// Simple check if description is likely to be multiline (more than ~80 characters)
	const isLongDescription = $derived(description && description.length > 80);
</script>

<div class="commit">
	{#if description}
		<div class="text-13 text-body description-container">
			<p
				data-testid={TestId.CommitDrawerDescription}
				class="description"
				class:truncated={isLongDescription && !showFullDescription}
			>
				{description}

				{#if isLongDescription && showFullDescription}
					<button
						type="button"
						class="fold-text-button"
						onclick={() => {
							showFullDescription = !showFullDescription;
						}}
					>
						show less
					</button>
				{/if}
			</p>
			{#if isLongDescription && !showFullDescription}
				<button
					type="button"
					class="fold-text-button truncated"
					onclick={() => {
						showFullDescription = !showFullDescription;
					}}
				>
					show more
				</button>
			{/if}
		</div>
	{/if}

	<div class="metadata text-12">
		<span>Author:</span>
		<Avatar
			size="medium"
			tooltip={commit.author.name}
			srcUrl={getGravatarUrl(commit.author.email, commit.author.gravatarUrl)}
		/>
		<span class="divider">•</span>
		<span>{getTimeAgo(new Date(commit.createdAt))}</span>
		<span class="divider">•</span>
		<Tooltip text="Copy commit SHA">
			<button
				type="button"
				class="copy-sha underline-dotted"
				onclick={() => {
					writeClipboard(commit.id, {
						message: 'Commit SHA copied'
					});
				}}
			>
				<span>
					{commit.id.substring(0, 7)}
				</span>
				<Icon name="copy-small" />
			</button>
		</Tooltip>
	</div>

	{#if !isUpstream}
		<div class="commit-details_actions">
			{@render children?.()}
		</div>
	{/if}
</div>

<style>
	.commit {
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

	.commit-details_actions {
		display: flex;
		flex-wrap: wrap;
		width: 100%;
		gap: 5px;
	}

	.copy-sha {
		display: flex;
		align-items: center;
		gap: 2px;
		text-decoration: underline dotted;
	}

	.description-container {
		position: relative;
	}

	.description {
		margin: 0;
		line-height: var(--text-lineheight-body);

		&.truncated {
			max-height: var(--text-lineheight-body); /* One line based on line-height */
			overflow: hidden;
			text-overflow: ellipsis;
			white-space: nowrap;
		}
	}

	.fold-text-button {
		background: var(--clr-bg-1);
		text-decoration: underline dotted;

		&::before {
			position: absolute;
			top: 0;
			left: -20px;
			width: 20px;
			height: 100%;
			background: linear-gradient(to right, transparent 0%, var(--clr-bg-1) 100%);
			content: '';
		}

		&.truncated {
			position: absolute;
			top: 0;
			right: 0;
			padding-left: 6px;
		}
	}
</style>

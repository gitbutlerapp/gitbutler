<script lang="ts">
	import { writeClipboard } from '$lib/backend/clipboard';
	import { type Commit, type UpstreamCommit } from '$lib/branches/v3';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { UserService } from '$lib/user/userService';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';

	type Props = {
		commit: UpstreamCommit | Commit;
	};

	const { commit }: Props = $props();

	const [userService] = inject(UserService, UiState);

	const user = $derived(userService.user);

	const message = $derived(commit.message);
	const { description } = $derived(splitMessage(message));

	function getGravatarUrl(email: string, existingGravatarUrl: string): string {
		if ($user?.email === undefined) {
			return existingGravatarUrl;
		}
		if (email === $user.email) {
			return $user.picture ?? existingGravatarUrl;
		}
		return existingGravatarUrl;
	}
</script>

<div class="commit">
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

	{#if description}
		<div
			class="text-13 text-body description-container"
			data-testid={TestId.CommitDrawerDescription}
		>
			<Markdown content={description} />
		</div>
	{/if}
</div>

<style>
	.commit {
		display: flex;
		flex-direction: column;
		gap: 12px;
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

	.copy-sha {
		display: flex;
		align-items: center;
		gap: 2px;
		text-decoration: underline dotted;
	}

	.description-container {
		position: relative;
	}
</style>

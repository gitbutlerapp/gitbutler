<script lang="ts">
	import { isCommit, type Commit, type UpstreamCommit } from '$lib/branches/v3';
	import { UiState } from '$lib/state/uiState.svelte';
	import { UserService } from '$lib/user/userService';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { inject } from '@gitbutler/shared/context';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import { marked } from '@gitbutler/ui/utils/marked';
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
	const description = $derived(splitMessage(message).description);
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
			{@render children?.()}
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

<script lang="ts">
	import { UserService } from '$lib/user/userService';
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import { marked } from '@gitbutler/ui/utils/marked';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';

	type Props = {
		projectId: string;
		commit: UpstreamCommit | Commit;
		href?: string;
		onclick?: () => void;
	};

	const { commit, onclick }: Props = $props();

	const [userService] = inject(UserService);
	const user = $derived(userService.user);

	const commitShortSha = $derived(commit.id.substring(0, 7));

	const message = $derived(commit.message);
	const description = $derived(message.slice(message.indexOf('\n') + 1).trim());

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

	{#if description}
		<div class="commit-description text-13">
			{@html marked(description)}
		</div>
	{/if}
</div>

<style>
	.commit-header {
		display: flex;
		flex-direction: column;
		gap: 8px;
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

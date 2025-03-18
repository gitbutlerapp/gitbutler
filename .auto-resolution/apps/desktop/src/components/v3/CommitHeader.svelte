<script lang="ts">
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { marked } from '@gitbutler/ui/utils/marked';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';
	import type { CommitKey } from '$lib/commits/commit';

	interface Props {
		projectId: string;
		commitKey: CommitKey;
		commit: UpstreamCommit | Commit;
		href?: string;
		onclick?: () => void;
	}

	const { commit, onclick }: Props = $props();

	const commitShortSha = $derived(commit.id.substring(0, 7));

	const message = $derived(commit.message);
	const title = $derived(message.slice(0, message.indexOf('\n')));
	const description = $derived(message.slice(message.indexOf('\n') + 1).trim());
</script>

<div class="commit-header" role="button" {onclick} onkeypress={onclick} tabindex="0">
	<div class="commit-title text-13 text-semibold">
		{title}
	</div>
	{#if description}
		<div class="commit-description text-12">
			{@html marked(description)}
		</div>
	{/if}
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
</div>

<style>
	.commit-header {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.commit-title {
		flex-grow: 1;
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
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

<script lang="ts">
	import { CLIPBOARD_SERVICE } from '$lib/backend/clipboard';
	import { type Commit, type UpstreamCommit } from '$lib/branches/v3';
	import { rewrapCommitMessage } from '$lib/config/uiFeatureFlags';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { USER_SERVICE } from '$lib/user/userService';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { rejoinParagraphs, truncate } from '$lib/utils/string';
	import { inject } from '@gitbutler/core/context';

	import { Avatar, Icon, TestId, TimeAgo, Tooltip } from '@gitbutler/ui';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';

	type Props = {
		commit: UpstreamCommit | Commit;
		rewrap?: boolean;
	};

	const { commit, rewrap }: Props = $props();

	const userService = inject(USER_SERVICE);
	const userSettings = inject(SETTINGS);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const zoom = $derived($userSettings.zoom);

	const user = $derived(userService.user);

	let messageWidth = $state(0);
	const messageWidthRem = $derived(pxToRem(messageWidth, zoom));

	// Calculate approximately how many characters fit on one line, as a
	// function of container width as well as zoom level.
	// TODO: Turn this magic formula into something meaningful.
	const fontFactor = $derived($rewrapCommitMessage ? 2.3 : 1.99);
	const maxLength = $derived((messageWidthRem - 2) * fontFactor - (Math.pow(zoom, 2) - 1));

	const message = $derived(commit.message);
	const raw = $derived(splitMessage(message).description);
	const description = $derived(rewrap ? rejoinParagraphs(raw) : raw);
	const abbreviated = $derived(truncate(description, maxLength, 3));
	const isAbbrev = $derived(abbreviated !== description);

	let expanded = $state(false);

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
			username={commit.author.name}
			srcUrl={getGravatarUrl(commit.author.email, commit.author.gravatarUrl)}
		/>
		<span class="divider">•</span>
		<TimeAgo date={new Date(commit.createdAt)} />
		<span class="divider">•</span>
		<Tooltip text="Copy commit SHA">
			<button
				type="button"
				class="copy-sha underline-dotted"
				onclick={() => {
					clipboardService.write(commit.id, {
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

	{#if description && description.trim()}
		<p
			class="description"
			class:expanded
			style:--commit-message-font={$rewrapCommitMessage
				? 'var(--fontfamily-default)'
				: 'var(--fontfamily-mono)'}
			bind:clientWidth={messageWidth}
			data-testid={TestId.CommitDrawerDescription}
		>
			{#if expanded}
				{description}
			{:else}
				{abbreviated}
			{/if}
			{#if isAbbrev}
				<button onclick={() => (expanded = !expanded)} type="button" class="readmore text-bold">
					{#if expanded}
						less
					{:else}
						more
					{/if}
				</button>
			{/if}
		</p>
	{/if}
</div>

<style>
	.commit {
		display: flex;
		flex-direction: column;
		padding: 14px;
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

	.description {
		font-size: 13px;
		line-height: var(--text-lineheight-body);
		font-family: var(--commit-message-font);
		white-space: pre-line;
	}

	.readmore {
		display: inline;
		position: relative;
		text-decoration: underline dotted;
	}
</style>

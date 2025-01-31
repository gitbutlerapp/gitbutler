<script lang="ts" module>
	export interface MessageProps {
		projectId: string;
		changeId?: string;
		event: ChatEvent;
	}
</script>

<script lang="ts">
	import MessageActions from './MessageActions.svelte';
	import { eventTimeStamp } from '$lib/chat/utils';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import type { ChatEvent } from '@gitbutler/shared/branches/types';

	const UNKNOWN_AUTHOR = 'Unknown author';

	const { event, projectId, changeId }: MessageProps = $props();

	const message = $derived(event.object);

	const authorName = $derived(
		message.user.login ?? message.user.name ?? message.user.email ?? UNKNOWN_AUTHOR
	);

	const timestamp = $derived(eventTimeStamp(event));
</script>

<div
	class="chat-message"
	class:open-issue={message.issue && !message.resolved}
	class:resolved={message.issue && message.resolved}
>
	{#if message.issue}
		<div class="chat-message__issue-icon" class:resolved={message.resolved}>
			<Icon name="warning-small" />
		</div>
	{:else}
		<img class="chat-message__avatar" src={message.user.avatarUrl} alt={authorName} />
	{/if}
	<div class="chat-message__data">
		<div class="chat-message__header">
			{#if message.issue}
				<img class="chat-message__avatar-small" src={message.user.avatarUrl} alt={authorName} />
			{/if}

			<div class="chat-message__author-name">
				{authorName}
			</div>

			{#if message.issue}
				{#if message.resolved}
					<Badge style="success">Issue resolved</Badge>
				{:else}
					<Badge style="error">Issue</Badge>
				{/if}
			{/if}

			<div class="chat-message__timestamp">
				{timestamp}
			</div>
		</div>

		<div class="chat-message-content">
			<div class="chat-message__content-text">
				<Markdown content={message.comment} />
			</div>

			<MessageActions {projectId} {changeId} {message} />
		</div>
	</div>
</div>

<style lang="postcss">
	.chat-message {
		display: flex;
		padding: 14px 16px;
		gap: 12px;

		background: var(--bg-1, #fff);
		border-bottom: 1px solid var(--clr-border-3, #eae9e8);

		&.open-issue {
			padding-left: 12px;
			border-left: 4px solid var(--clr-scale-err-50, #dc606b);
		}

		&.resolved {
			padding-left: 12px;
			border-left: 4px solid var(--clr-core-ntrl-60, #b4afac);
		}
	}

	.chat-message__issue-icon {
		display: flex;
		width: 24px;
		height: 24px;
		padding: 4px;
		justify-content: center;
		align-items: center;
		flex-shrink: 0;

		border-radius: 8px;
		background: var(--clr-scale-err-50, #dc606b);
		color: var(--clr-theme-err-soft);

		&.resolved {
			background: var(--clr-core-ntrl-60, #b4afac);
			color: var(--clr-core-ntrl-100, #d4d0ce);
		}
	}

	.chat-message__avatar {
		width: 24px;
		height: 24px;
		border-radius: 50%;
	}

	.chat-message__avatar-small {
		width: 16px;
		height: 16px;
		border-radius: 20px;
	}

	.chat-message__data {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.chat-message__header {
		margin-top: 4px;
		display: flex;
		gap: 7px;
	}

	.chat-message__timestamp {
		overflow: hidden;
		color: var(--clr-text-1, #1a1614);
		text-overflow: ellipsis;

		/* base/12 */
		font-family: var(--text-fontfamily-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--text-weight-regular, 400);
		line-height: 120%; /* 14.4px */

		opacity: 0.4;
	}

	.chat-message__author-name {
		overflow: hidden;
		color: var(--text-1, #1a1614);
		text-overflow: ellipsis;

		/* base/13-bold */
		font-family: var(--text-fontfamily-default, Inter);
		font-size: 13px;
		font-style: normal;
		font-weight: 600;
		line-height: 120%; /* 15.6px */
	}

	.chat-message-content {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 16px;
		align-self: stretch;
	}

	.chat-message__content-text {
		color: var(--text-1, #1a1614);

		/* base-body/13 */
		font-family: var(--text-fontfamily-default, Inter);
		font-size: 13px;
		font-style: normal;
		font-weight: var(--text-weight-regular, 400);
		line-height: 160%; /* 20.8px */
	}
</style>

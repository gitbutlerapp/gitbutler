<script lang="ts" module>
	export interface MessageProps {
		highlight?: boolean;
		projectId: string;
		changeId?: string;
		event: ChatEvent;
	}
</script>

<script lang="ts">
	import MessageActions from './MessageActions.svelte';
	import MessageMarkdown from './MessageMarkdown.svelte';
	import { eventTimeStamp } from '@gitbutler/shared/branches/utils';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { ChatEvent } from '@gitbutler/shared/branches/types';

	const UNKNOWN_AUTHOR = 'Unknown author';

	const { event, projectId, changeId, highlight }: MessageProps = $props();

	const message = $derived(event.object);

	const authorName = $derived(
		message.user.login ?? message.user.name ?? message.user.email ?? UNKNOWN_AUTHOR
	);

	const timestamp = $derived(eventTimeStamp(event));
</script>

<div
	id="chat-message-{message.uuid}"
	class="chat-message"
	class:highlight
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

			<div class="text-13 text-bold chat-message__author-name">
				{authorName}
			</div>

			{#if message.issue}
				{#if message.resolved}
					<Badge style="success">Issue resolved</Badge>
				{:else}
					<Badge style="warning">Issue</Badge>
				{/if}
			{/if}

			<div class="text-12 chat-message__timestamp" title={event.createdAt}>
				{timestamp}
			</div>
		</div>

		<div class="chat-message-content">
			<div class="text-13 text-body chat-message__content-text">
				<MessageMarkdown content={message.comment} mentions={message.mentions} />
			</div>

			<MessageActions {projectId} {changeId} {message} />
		</div>
	</div>
</div>

<style lang="postcss">
	@keyframes temporary-highlight {
		0% {
			background: var(--clr-bg-1-muted);
		}
		75% {
			background: var(--clr-bg-1-muted);
		}
		100% {
			background: var(--clr-bg-1);
		}
	}
	.chat-message {
		display: flex;
		padding: 14px 16px;
		gap: 12px;

		background: var(--clr-bg-1);
		border-bottom: 1px solid var(--clr-border-3);

		&:first-child {
			border-bottom: none;
		}

		&.open-issue {
			padding-left: 12px;
			border-left: 4px solid var(--clr-br-commit-changes-requested-bg);
		}

		&.resolved {
			padding-left: 12px;
			border-left: 4px solid var(--clr-core-ntrl-60);
		}

		&.highlight {
			animation: temporary-highlight 2s ease-out;
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
		background: var(--clr-br-commit-changes-requested-bg);
		color: var(--clr-br-commit-changes-requested-text);

		&.resolved {
			background: var(--clr-core-ntrl-60);
			color: var(--clr-core-ntrl-100);
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
		color: var(--clr-text-1);
		text-overflow: ellipsis;
		opacity: 0.4;
	}

	.chat-message__author-name {
		overflow: hidden;
		color: var(--clr-text-1);
		text-overflow: ellipsis;
	}

	.chat-message-content {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 16px;
		align-self: stretch;
	}

	.chat-message__content-text {
		color: var(--clr-text-1);
	}
</style>

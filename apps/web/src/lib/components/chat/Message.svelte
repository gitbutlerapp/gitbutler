<script lang="ts" module>
	import { messageTimeStamp } from '$lib/chat/utils';
	import type { ChatMessage } from '@gitbutler/shared/chat/types';

	export interface MessageProps {
		message: ChatMessage;
	}
</script>

<script lang="ts">
	const UNKNOWN_AUTHOR = 'Unknown author';

	const { message }: MessageProps = $props();

	const authorName = $derived(
		message.user.login ?? message.user.name ?? message.user.email ?? UNKNOWN_AUTHOR
	);

	const timestamp = $derived(messageTimeStamp(message));
</script>

<div class="chat-message">
	<img class="chat-message__subtitle-avatar" src={message.user.avatarUrl} alt={authorName} />
	<div class="chat-message__content">
		<div class="chat-message__header">
			<div class="chat-message__author-name">
				{authorName}
			</div>

			<div class="chat-message__timestamp">
				{timestamp}
			</div>
		</div>
		<p class="chat-message__content-text">
			{message.comment}
		</p>
	</div>
</div>

<style>
	.chat-message {
		display: flex;
		padding: 14px 16px;
		gap: 12px;

		background: var(--bg-1, #fff);
		border-bottom: 1px solid var(--border-2, #d4d0ce);
	}

	.chat-message__subtitle-avatar {
		width: 24px;
		height: 24px;
		border-radius: 50%;
	}

	.chat-message__content {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.chat-message__header {
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

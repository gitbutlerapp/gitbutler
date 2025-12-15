<script lang="ts" module>
	export type ReplyToMessage = {
		uuid: string;
		user: UserSimple;
		comment: string;
	};
</script>

<script lang="ts">
	import { Button } from '@gitbutler/ui';
	import type { UserSimple } from '@gitbutler/shared/users/types';

	const UNKNOWN_AUTHOR = 'Unknown author';

	type Props = {
		message: ReplyToMessage;
		clickable?: boolean;
		clearReply?: () => void;
	};

	const { message, clearReply, clickable }: Props = $props();

	const authorName = $derived(
		message.user.login ?? message.user.name ?? message.user.email ?? UNKNOWN_AUTHOR
	);
</script>

<div class="reply" class:clickable>
	<div class="reply__content">
		<img class="reply__avatar" src={message.user.avatarUrl} alt={authorName} />
		<div class="reply__text">
			<span class="text-11 text-body">
				{message.comment}
			</span>
		</div>
	</div>

	{#if clearReply}
		<Button icon="cross" style="neutral" kind="ghost" size="tag" onclick={clearReply} />
	{/if}
</div>

<style lang="postcss">
	.reply {
		display: flex;
		align-items: center;
		align-self: stretch;
		justify-content: space-between;
		overflow: hidden;

		border-radius: var(--radius-m);
		background: var(--clr-bg-2);

		&.clickable:hover {
			background: var(--clr-bg-2-muted);
		}
	}

	.reply__content {
		display: flex;
		align-items: center;
		padding: 8px;
		overflow: hidden;
		gap: 8px;
		border-left: 4px solid var(--clr-border-2);
	}

	.reply__text {
		display: flex;
		max-height: 1em;
		overflow: hidden;

		& > span {
			display: -webkit-inline-box;
			-webkit-line-clamp: 1;
			-webkit-box-orient: vertical;
			overflow: hidden;
			text-overflow: ellipsis;
			white-space: break-spaces;
			word-break: break-all;
		}
	}

	.reply__avatar {
		width: 16px;
		height: 16px;
		border-radius: 20px;
	}
</style>

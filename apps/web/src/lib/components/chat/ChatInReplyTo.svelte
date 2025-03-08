<script lang="ts" module>
	export type ReplyToMessage = {
		uuid: string;
		user: UserSimple;
		comment: string;
	};
</script>

<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';
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
		<p class="text-11 text-body reply_text">
			{message.comment}
		</p>
	</div>

	{#if clearReply}
		<Button icon="cross" style="neutral" kind="ghost" size="tag" onclick={clearReply} />
	{/if}
</div>

<style lang="postcss">
	.reply {
		display: flex;
		padding: 0 8px 0 0;
		align-self: stretch;
		justify-content: space-between;
		align-items: center;
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
		gap: 7px;

		padding: 8px 0 8px 12px;

		border-left: 4px solid var(--clr-scale-ntrl-60);
	}

	.reply_text {
		margin: 0;
		text-overflow: ellipsis;
		text-wrap: nowrap;
		overflow-x: hidden;
		max-width: 320px;
	}

	.reply__avatar {
		width: 16px;
		height: 16px;
		border-radius: 20px;
	}
</style>

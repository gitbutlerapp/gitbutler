<script lang="ts" module>
	export interface MessageProps {
		highlight?: boolean;
		projectId: string;
		changeId?: string;
		event: ChatEvent;
		disableActions?: boolean;
		onReply: () => void;
		scrollToMessage: (uuid: string) => void;
	}
</script>

<script lang="ts">
	import ChatInReplyTo from './ChatInReplyTo.svelte';
	import MessageActions from './MessageActions.svelte';
	import MessageDiffSection from './MessageDiffSection.svelte';
	import MessageMarkdown from './MessageMarkdown.svelte';
	import { parseDiffPatchToContentSection } from '$lib/chat/diffPatch';
	import { updateReactions } from '$lib/chat/reactions';
	import { parseDiffPatchToEncodedSelection } from '$lib/diff/lineSelection.svelte';
	import { UserService } from '$lib/user/userService';
	import { eventTimeStamp } from '@gitbutler/shared/branches/utils';
	import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
	import { type ChatMessageReaction } from '@gitbutler/shared/chat/types';
	import { getContext } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import {
		findEmojiByUnicode,
		getInitialEmojis,
		markRecentlyUsedEmoji,
		type EmojiInfo
	} from '@gitbutler/ui/emoji/utils';
	import PopoverActionsContainer from '@gitbutler/ui/popoverActions/PopoverActionsContainer.svelte';
	import PopoverActionsItem from '@gitbutler/ui/popoverActions/PopoverActionsItem.svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import type { ChatEvent } from '@gitbutler/shared/patchEvents/types';
	import type { UserSimple } from '@gitbutler/shared/users/types';

	const UNKNOWN_AUTHOR = 'Unknown author';

	const {
		event,
		projectId,
		changeId,
		highlight,
		disableActions,
		onReply,
		scrollToMessage
	}: MessageProps = $props();

	const chatChannelService = getContext(ChatChannelsService);
	const userService = getContext(UserService);
	const user = $derived(userService.user);

	let kebabMenuTrigger = $state<HTMLButtonElement>();
	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let isOpenedByKebabButton = $state(false);
	let recentlyUsedEmojis = $state<EmojiInfo[]>([]);
	const reactionSet = new SvelteSet<string>();

	const message = $derived(event.object);
	let optimisticEmojiReactions = $state<ChatMessageReaction[]>();

	$effect(() => {
		if (message.emojiReactions) {
			optimisticEmojiReactions = message.emojiReactions;
		}
	});

	const authorName = $derived(
		message.user.login ?? message.user.name ?? message.user.email ?? UNKNOWN_AUTHOR
	);

	const timestamp = $derived(eventTimeStamp(event));

	const content = $derived(parseDiffPatchToContentSection(message.diffPatchArray));
	const diffSelectionString = $derived.by(() => {
		if (message.diffPatchArray === undefined || message.diffPath === undefined) return undefined;
		return parseDiffPatchToEncodedSelection(message.diffPath, message.diffPatchArray);
	});

	function handleGoToDiff() {
		if (!diffSelectionString) return;
		const rowElement = document.getElementById(`hunk-line-${diffSelectionString}`);
		if (rowElement) rowElement.scrollIntoView({ behavior: 'smooth', block: 'center' });
	}

	function setRecentlyUsedEmojis() {
		const emojis = getInitialEmojis();
		recentlyUsedEmojis = emojis.slice(0, 3);
	}

	function reactButDoItOptimisticaly(emoji: EmojiInfo) {
		if (!$user || !optimisticEmojiReactions) return;
		const newReactions = structuredClone($state.snapshot(optimisticEmojiReactions));
		optimisticEmojiReactions = updateReactions($user, emoji, newReactions);
	}

	async function handleReaction(emoji: EmojiInfo) {
		if (reactionSet.has(emoji.unicode)) return;
		reactionSet.add(emoji.unicode);
		try {
			await chatChannelService.patchChatMessage({
				projectId,
				changeId,
				messageUuid: message.uuid,
				reaction: emoji.unicode
			});
			reactButDoItOptimisticaly(emoji);
			markRecentlyUsedEmoji(emoji);
		} catch (error) {
			console.error('Failed to add reaction', error);
		}
		reactionSet.delete(emoji.unicode);
	}

	async function handleClickOnExistingReaction(unicode: string) {
		const emojiInfo = findEmojiByUnicode(unicode);
		if (!emojiInfo) return;
		await handleReaction(emojiInfo);
	}

	function getReactionTooltip(users: UserSimple[]) {
		const thisUsername = $user?.login;
		if (users.length === 0) return '';
		const formatted = users.map((user) => (user.login === thisUsername ? 'You' : user.login));
		if (formatted.length < 4) return formatted.map((user) => user).join(', ');
		return (
			formatted
				.slice(0, 3)
				.map((user) => user)
				.join(', ') + ` and ${formatted.length - 3} more`
		);
	}

	function thisUserReacted(users: UserSimple[]) {
		const thisUsername = $user?.login;
		return users.some((user) => user.login === thisUsername);
	}
</script>

<div
	role="listitem"
	id="chat-message-{message.uuid}"
	class="chat-message"
	class:highlight
	class:open-issue={message.issue && !message.resolved}
	class:resolved={message.issue && message.resolved}
	onmouseenter={setRecentlyUsedEmojis}
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

		{#if message.inReplyTo}
			<button type="button" onclick={() => scrollToMessage(message.inReplyTo!.uuid)}>
				<ChatInReplyTo message={message.inReplyTo} clickable />
			</button>
		{/if}

		{#if message.diffPatchArray && message.diffPatchArray.length > 0 && message.diffPath}
			<MessageDiffSection diffPath={message.diffPath} {content} onGoToDiff={handleGoToDiff} />
		{/if}

		<div class="chat-message-content">
			<div class="text-13 text-body chat-message__content-text">
				<MessageMarkdown content={message.comment} mentions={message.mentions} />
			</div>

			<MessageActions {projectId} {changeId} {message} />
		</div>

		{#if optimisticEmojiReactions && optimisticEmojiReactions.length > 0}
			<div class="chat-message__reactions">
				{#each optimisticEmojiReactions as reaction}
					{@const reacted = thisUserReacted(reaction.users)}
					<Button
						style="neutral"
						kind={reacted ? 'ghost' : 'outline'}
						size="tag"
						loading={reactionSet.has(reaction.reaction)}
						disabled={!$user}
						tooltip={getReactionTooltip(reaction.users)}
						customStyle={reacted ? 'background: var(--clr-theme-pop-soft);' : undefined}
						onclick={() => handleClickOnExistingReaction(reaction.reaction)}
					>
						<div class="text-13">
							{reaction.reaction + ' ' + reaction.users.length}
						</div>
					</Button>
				{/each}
			</div>
		{/if}
	</div>

	<!-- Message actions -->
	{#if !disableActions}
		<PopoverActionsContainer class="message-actions-menu" thin stayOpen={isOpenedByKebabButton}>
			<!-- Emoji Reactions -->
			{#if recentlyUsedEmojis.length > 0}
				{#each recentlyUsedEmojis as emoji}
					<PopoverActionsItem
						tooltip={emoji.label}
						thin
						disabled={!$user || reactionSet.has(emoji.unicode)}
						onclick={() => handleReaction(emoji)}
					>
						<p class="text-13" style="padding: 2px;">
							{emoji.unicode}
						</p>
					</PopoverActionsItem>
				{/each}
			{/if}

			<!-- Reply -->
			<PopoverActionsItem icon="reply" tooltip="Reply" thin onclick={() => onReply()} />

			<!-- Kebab menu -->
			<PopoverActionsItem
				bind:el={kebabMenuTrigger}
				activated={isOpenedByKebabButton}
				icon="kebab"
				tooltip="More options"
				thin
				disabled
				onclick={() => {
					contextMenu?.toggle();
				}}
			/>
		</PopoverActionsContainer>
	{/if}
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
		position: relative;
		width: 100%;
		display: flex;
		padding: 14px 16px;
		gap: 12px;
		box-sizing: border-box;
		flex-shrink: 0;

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

		&:hover :global(.message-actions-menu) {
			--show: true;
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
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 12px;
		flex-grow: 0;
		min-width: 0;
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
		width: 100%;
		box-sizing: border-box;
	}

	.chat-message__content-text {
		color: var(--clr-text-1);
		width: 100%;
		box-sizing: border-box;
	}

	.chat-message__reactions {
		display: flex;
		gap: 2px;
	}
</style>

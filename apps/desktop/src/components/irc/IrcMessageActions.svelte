<script lang="ts">
	import { ContextMenu, EmojiPicker } from "@gitbutler/ui";
	import {
		getInitialEmojis,
		markRecentlyUsedEmoji,
		type EmojiInfo,
	} from "@gitbutler/ui/components/emoji/utils";
	import type { StoredMessage } from "$lib/irc/ircApi";

	const QUICK_EMOJI_COUNT = 4;

	type Props = {
		msg: StoredMessage;
		isOwn?: boolean;
		onReply?: (msg: StoredMessage) => void;
		onReact: (msg: StoredMessage, emoji: string) => void;
		onDelete?: (msg: StoredMessage) => void;
		onPickerToggle?: (isOpen: boolean) => void;
	};

	const { msg, isOwn, onReply, onReact, onDelete, onPickerToggle }: Props = $props();

	const quickEmojis = $derived(getInitialEmojis().slice(0, QUICK_EMOJI_COUNT));

	let pickerMenu = $state<ReturnType<typeof ContextMenu>>();
	let pickerTrigger = $state<HTMLButtonElement>();

	function handleEmojiPick(emoji: EmojiInfo) {
		markRecentlyUsedEmoji(emoji);
		onReact(msg, emoji.unicode);
		pickerMenu?.close();
	}
</script>

<div class="message-actions" role="toolbar" tabindex="0">
	{#if onReply}
		<button type="button" class="action-btn" title="Reply" onclick={() => onReply(msg)}>↩</button>
	{/if}
	{#each quickEmojis as emoji}
		<button
			type="button"
			class="action-btn"
			title={emoji.label}
			onclick={() => {
				markRecentlyUsedEmoji(emoji);
				onReact(msg, emoji.unicode);
			}}>{emoji.unicode}</button
		>
	{/each}
	<button
		type="button"
		class="action-btn"
		title="More reactions"
		bind:this={pickerTrigger}
		onclick={() => pickerMenu?.toggle()}>+</button
	>
	{#if isOwn && onDelete}
		<button
			type="button"
			class="action-btn delete-btn"
			title="Delete message"
			onclick={() => onDelete(msg)}>🗑</button
		>
	{/if}
</div>

<ContextMenu
	bind:this={pickerMenu}
	leftClickTrigger={pickerTrigger}
	side="top"
	align="end"
	ontoggle={(isOpen) => {
		onPickerToggle?.(isOpen);
	}}
>
	<EmojiPicker onEmojiSelect={handleEmojiPick} />
</ContextMenu>

<style lang="postcss">
	.message-actions {
		display: flex;
		gap: 2px;
		border: 1px solid var(--clr-bg-3);
		border-radius: 6px;
		background-color: var(--clr-bg-muted);
	}
	.action-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 24px;
		padding: 0;
		border: none;
		border-radius: 4px;
		background: none;
		color: var(--clr-text-2);
		font-size: 14px;
		cursor: pointer;
		&:hover {
			background-color: var(--clr-bg-2);
		}
		&.delete-btn:hover {
			color: var(--clr-theme-err-element);
		}
	}
</style>

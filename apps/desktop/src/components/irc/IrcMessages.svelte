<script lang="ts">
	import IrcCommit from "$components/irc/IrcCommit.svelte";
	import IrcHunk from "$components/irc/IrcHunk.svelte";
	import IrcMessageActions from "$components/irc/IrcMessageActions.svelte";
	import { parseMessageData } from "$lib/irc/protocol";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { inject } from "@gitbutler/core/context";
	import {
		VirtualList,
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		Avatar,
		Markdown,
	} from "@gitbutler/ui";
	import { onDestroy } from "svelte";
	import type { Reaction, StoredMessage } from "$lib/irc/ircEndpoints";

	function avatarUrl(sender: string): string {
		return `https://github.com/${encodeURIComponent(sender)}.png?size=64`;
	}

	const HALF_DAY_MS = 12 * 60 * 60 * 1000;

	function formatTimestamp(ts: number): string {
		const date = new Date(ts);
		const age = Date.now() - ts;
		if (age < HALF_DAY_MS) {
			return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
		}
		return (
			date.toLocaleDateString([], { month: "numeric", day: "numeric" }) +
			" " +
			date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
		);
	}

	interface MessageGroup {
		sender: string;
		isSystem: boolean;
		messages: StoredMessage[];
	}

	const GROUP_TIME_LIMIT_MS = 5 * 60 * 1000; // 5 minutes

	function groupConsecutive(msgs: StoredMessage[]): MessageGroup[] {
		const groups: MessageGroup[] = [];
		for (const msg of msgs) {
			const isSystem = msg.sender === "*";
			const last = groups[groups.length - 1];
			const lastMsg = last?.messages.at(-1);
			const withinTimeLimit =
				lastMsg !== undefined && msg.timestamp - lastMsg.timestamp < GROUP_TIME_LIMIT_MS;
			if (last && last.sender === msg.sender && last.isSystem === isSystem && withinTimeLimit) {
				last.messages.push(msg);
			} else {
				groups.push({ sender: msg.sender, isSystem, messages: [msg] });
			}
		}
		return groups;
	}

	type Props = {
		projectId: string;
		messages: StoredMessage[];
		myNick?: string;
		messageReactions: Record<string, Reaction[]>;
		typingUsers?: string[];
		bannerText?: string;
		editorName?: string;
		/** Return true if more history may exist, false when the top has been reached. */
		onLoadMore?: () => Promise<boolean>;
		onReply?: (msg: StoredMessage) => void;
		onToggleReaction?: (msg: StoredMessage, emoji: string) => void;
		onDeleteMessage?: (msg: StoredMessage) => void;
		onOpenInEditor?: (filePath: string, lineNumber?: number) => void;
	};

	const {
		projectId,
		messages,
		myNick,
		messageReactions,
		typingUsers,
		bannerText,
		editorName,
		onLoadMore,
		onReply,
		onToggleReaction,
		onDeleteMessage,
		onOpenInEditor,
	}: Props = $props();

	const grouped = $derived(groupConsecutive(messages));

	// True when there is no more history to load — banner is shown only then.
	let reachedTop = $state(!onLoadMore);

	async function handleLoadMore() {
		const hasMore = await onLoadMore?.();
		if (hasMore === false) {
			reachedTop = true;
		}
	}

	const userSettings = inject(SETTINGS);

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let messagesEl = $state<HTMLDivElement>();

	// ── Single shared IrcMessageActions instance ────────────────────────
	let hoveredMsg = $state<StoredMessage | null>(null);
	let hoveredRowEl = $state<HTMLElement | null>(null);
	let actionsEl = $state<HTMLDivElement | null>(null);
	let pickerHolding = $state(false);
	const activeMsg = $derived(hoveredMsg);
	const activeMessageId = $derived(activeMsg?.msgid ?? null);

	// Position of the actions toolbar relative to .messages container.
	let actionsStyle = $state("");

	function updateActionsPosition(rowEl: HTMLElement | null) {
		if (!rowEl || !messagesEl) {
			actionsStyle = "display:none";
			return;
		}
		const containerRect = messagesEl.getBoundingClientRect();
		const rowRect = rowEl.getBoundingClientRect();
		const top = rowRect.top - containerRect.top - 18;
		const right = 20;
		actionsStyle = `top:${top}px;right:${right}px`;
	}

	$effect(() => {
		updateActionsPosition(hoveredRowEl);
	});

	function onRowEnter(msg: StoredMessage, rowEl: HTMLElement) {
		if (scrolling || pickerHolding) return;
		hoveredMsg = msg;
		hoveredRowEl = rowEl;
	}

	function onRowLeave(e: PointerEvent) {
		if (pickerHolding) return;
		// Don't dismiss if the pointer moved into the floating actions bar.
		const related = e.relatedTarget;
		if (related instanceof Node && actionsEl?.contains(related)) return;
		hoveredMsg = null;
		hoveredRowEl = null;
	}

	function onActionsLeave(e: PointerEvent) {
		if (pickerHolding) return;
		// Don't dismiss if the pointer moved back to the active row.
		const related = e.relatedTarget;
		if (related instanceof Node && hoveredRowEl?.contains(related)) return;
		hoveredMsg = null;
		hoveredRowEl = null;
	}

	function onPickerToggle(isOpen: boolean) {
		pickerHolding = isOpen;
		if (!isOpen) {
			hoveredMsg = null;
			hoveredRowEl = null;
		}
	}

	let scrolling = $state(false);
	let scrollTimer: ReturnType<typeof setTimeout>;
	function onScroll(event: Event) {
		// Ignore scroll events from nested scrollable elements (e.g. IrcHunk, IrcCommit).
		if (event.target instanceof HTMLElement && event.target.closest(".message-row")) return;
		scrolling = true;
		clearTimeout(scrollTimer);
		scrollTimer = setTimeout(() => {
			scrolling = false;
		}, 150);
		// Dismiss actions on scroll unless the picker is holding.
		if (!pickerHolding) {
			hoveredMsg = null;
			hoveredRowEl = null;
		} else if (hoveredRowEl) {
			updateActionsPosition(hoveredRowEl);
		}
	}
	onDestroy(() => clearTimeout(scrollTimer));

	function groupReactions(
		reactions: { sender: string; reaction: string }[],
	): { emoji: string; count: number; senders: string[] }[] {
		const map = new Map<string, string[]>();
		for (const r of reactions) {
			const senders = map.get(r.reaction) ?? [];
			senders.push(r.sender);
			map.set(r.reaction, senders);
		}
		return Array.from(map.entries()).map(([emoji, senders]) => ({
			emoji,
			count: senders.length,
			senders,
		}));
	}
</script>

{#snippet singleMessage(msg: StoredMessage, isFirst: boolean)}
	{@const timestamp = formatTimestamp(msg.timestamp)}
	{@const data = msg.data ? parseMessageData(msg.data) : undefined}
	{@const parentMsg = msg.replyTo ? messages.find((m) => m.msgid === msg.replyTo) : undefined}
	{@const reactions = groupReactions(msg.msgid ? (messageReactions[msg.msgid] ?? []) : [])}
	{@const displayContent = msg.content.trim() ? msg.content : "\u00a0"}
	<div
		class="message-row"
		class:active={activeMessageId === msg.msgid}
		onpointerenter={(e) => {
			if (msg.msgid) onRowEnter(msg, e.currentTarget);
		}}
		onpointerleave={(e) => onRowLeave(e)}
	>
		{#if isFirst}
			<div class="avatar-wrapper">
				<Avatar srcUrl={avatarUrl(msg.sender)} username={msg.sender} size="large" />
			</div>
		{:else}
			<div class="gutter">
				<span class="hover-time text-9">{timestamp}</span>
			</div>
		{/if}
		<div class="message-body">
			{#if parentMsg}
				<div class="reply-context text-11">
					reply to {parentMsg.sender}: {parentMsg.content.length > 80
						? parentMsg.content.slice(0, 80) + "..."
						: parentMsg.content}
				</div>
			{/if}
			{#if isFirst}
				<div class="message-header">
					<span class="sender text-13">{msg.sender}</span>
					<span class="timestamp text-11">{timestamp}</span>
				</div>
			{/if}
			{#if data?.type === "shared-commit"}
				<IrcCommit {projectId} commit={data.commit} />
			{:else if data?.type === "shared-hunk" && data.diff.diff}
				<IrcHunk
					change={data.change}
					diff={data.diff}
					onLineContextMenu={({ event, target, filePath, lineNumber }) => {
						contextMenu?.open(event || target, { filePath, lineNumber });
					}}
				/>
			{:else}
				<div class="message-content text-13 text-body">
					<Markdown content={displayContent} />
				</div>
			{/if}
			{#if reactions.length > 0}
				<div class="message-reactions">
					{#each reactions as group}
						{@const isOwn = !!myNick && group.senders.includes(myNick)}
						<button
							type="button"
							class="reaction-pill"
							class:own={isOwn}
							title={group.senders.join(", ")}
							onclick={() => onToggleReaction?.(msg, group.emoji)}
							>{group.emoji} {group.count}</button
						>
					{/each}
				</div>
			{/if}
		</div>
	</div>
{/snippet}

{#snippet groupTemplate(group: MessageGroup)}
	{#if group.isSystem}
		{#each group.messages as msg}
			{@const timestamp = formatTimestamp(msg.timestamp)}
			<div class="system-message text-11 text-body">
				<span class="system-time">{timestamp}</span>
				{#if msg.tag}<span class="msg-tag">[{msg.tag}]</span>{/if}
				{msg.content}
			</div>
		{/each}
	{:else}
		<div class="message-group">
			{#each group.messages as msg, i}
				{@render singleMessage(msg, i === 0)}
			{/each}
		</div>
	{/if}
{/snippet}

<div
	class="messages"
	class:scrolling
	class:picker-open={pickerHolding}
	bind:this={messagesEl}
	onscrollcapture={onScroll}
>
	<VirtualList
		items={grouped}
		defaultHeight={45}
		stickToBottom
		showBottomButton
		onloadmore={onLoadMore ? handleLoadMore : undefined}
		renderDistance={100}
		loadMoreThreshold={500}
		visibility={$userSettings.scrollbarVisibilityState}
		getId={(group?: MessageGroup) => group?.messages[0]?.msgid}
	>
		{#snippet banner()}
			{#if bannerText && reachedTop}
				<div class="channel-banner text-12">
					{bannerText}
				</div>
			{/if}
		{/snippet}
		{#snippet template(group)}
			{@render groupTemplate(group)}
		{/snippet}
	</VirtualList>
	{#if activeMsg}
		<div
			class="floating-actions"
			style={actionsStyle}
			bind:this={actionsEl}
			onpointerleave={(e) => onActionsLeave(e)}
		>
			<IrcMessageActions
				msg={activeMsg}
				isOwn={!!myNick && activeMsg.sender === myNick}
				{onReply}
				onReact={(m, emoji) => onToggleReaction?.(m, emoji)}
				onDelete={(m) => onDeleteMessage?.(m)}
				{onPickerToggle}
			/>
		</div>
	{/if}
</div>
{#if typingUsers && typingUsers.length > 0}
	<div class="typing-indicator text-11">
		{#if typingUsers.length === 1}
			<span class="typing-nick">{typingUsers[0]}</span> is typing
		{:else if typingUsers.length === 2}
			<span class="typing-nick">{typingUsers[0]}</span> and
			<span class="typing-nick">{typingUsers[1]}</span> are typing
		{:else}
			<span class="typing-nick">{typingUsers[0]}</span> and {typingUsers.length - 1} others are typing
		{/if}
		<span class="typing-dots">...</span>
	</div>
{/if}

{#if onOpenInEditor}
	<ContextMenu bind:this={contextMenu} rightClickTrigger={messagesEl} align="start" side="right">
		{#snippet children(item)}
			<ContextMenuSection>
				<ContextMenuItem
					label="Open in {editorName ?? 'Editor'}"
					icon="open-in-ide"
					onclick={() => {
						if (item.filePath) {
							onOpenInEditor(item.filePath, item.lineNumber);
						}
						contextMenu?.close();
					}}
				/>
			</ContextMenuSection>
		{/snippet}
	</ContextMenu>
{/if}

<style lang="postcss">
	.messages {
		position: relative;
		flex-grow: 1;
		overflow: hidden;
		background-color: var(--bg-1);
	}
	.message-group {
		padding: 4px 0;
		background-color: transparent;
		user-select: text;
	}
	.message-row {
		display: flex;
		position: relative;
		max-width: calc(100% - 12px);
		padding: 2px 12px;
		gap: 10px;
		border-radius: var(--radius-s);
	}
	.messages:not(.scrolling):not(.picker-open) .message-row:hover,
	.message-row.active {
		background-color: var(--bg-mute);
	}
	.messages.picker-open :global(.padded-contents) {
		pointer-events: none;
	}
	.floating-actions {
		z-index: 2;
		position: absolute;
		pointer-events: auto;
	}
	.floating-actions :global(.message-actions) {
		display: flex;
		position: static;
	}
	.avatar-wrapper {
		flex-shrink: 0;
		width: 28px;
		padding-top: 2px;
	}
	.gutter {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 28px;
	}
	.hover-time {
		display: none;
		overflow: hidden;
		color: var(--text-3);
		font-size: 9px;
		line-height: 1;
	}
	.message-row:hover .hover-time {
		display: block;
	}
	.message-body {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		min-width: 0;
		gap: 4px;
	}
	.message-header {
		display: flex;
		align-items: baseline;
		gap: 6px;
	}
	.sender {
		color: var(--text-1);
		font-weight: 600;
	}
	.timestamp {
		color: var(--text-3);
	}
	.message-content {
		display: flex;
		flex-direction: column;
		gap: 8px;
		color: var(--text-2);
		word-break: break-word;

		:global(.markdown) {
			display: flex;
			flex-direction: column;
			gap: 4px;
		}

		:global(.markdown p) {
			margin: 0;
		}

		:global(.markdown pre) {
			padding: 6px 8px;
			overflow-x: auto;
			border-radius: var(--radius-s);
			background-color: var(--bg-2);
		}

		:global(.markdown code) {
			padding: 1px 4px;
			border-radius: var(--radius-s);
			background-color: var(--bg-2);
			font-size: 12px;
			font-family: var(--font-mono);
		}

		:global(.markdown pre code) {
			padding: 0;
			background: none;
		}
	}
	.message-reactions {
		display: flex;
		align-items: center;
		margin-top: 2px;
		gap: 4px;
	}
	.reaction-pill {
		display: inline-flex;
		align-items: center;
		padding: 2px 6px;
		gap: 2px;
		border: 1px solid transparent;
		border-radius: 10px;
		background-color: var(--bg-2);
		font-size: 12px;
		cursor: pointer;
		&:hover {
			background-color: var(--bg-3);
		}
		&.own {
			border-color: var(--fill-pop-bg);
			background-color: color-mix(in srgb, var(--fill-pop-bg) 15%, transparent);
		}
	}
	.reply-context {
		margin-bottom: 2px;
		padding-left: 6px;
		overflow: hidden;
		border-left: 2px solid var(--border-2);
		color: var(--text-3);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.system-message {
		padding: 0 12px;
		padding-left: 24px;
		color: var(--text-2);
		line-height: 1;
		font-family: var(--font-mono);
		text-indent: -12px;
		white-space: pre-wrap;
		user-select: text;
	}
	.system-time {
		color: var(--text-3);
		opacity: 0.6;
	}
	.msg-tag {
		color: var(--text-2);
		opacity: 0.7;
	}
	.channel-banner {
		padding: 16px 12px;
		color: var(--text-3);
		text-align: center;
	}
	.typing-indicator {
		flex-shrink: 0;
		padding: 0 12px 4px 12px;
		color: var(--text-3);
	}
	.typing-nick {
		font-weight: 600;
	}
	@keyframes typing-blink {
		0%,
		100% {
			opacity: 0.3;
		}
		50% {
			opacity: 1;
		}
	}
	.typing-dots {
		animation: typing-blink 1.2s ease-in-out infinite;
	}
</style>

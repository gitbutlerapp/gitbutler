<script lang="ts">
	import IrcCommit from "$components/IrcCommit.svelte";
	import IrcHunk from "$components/IrcHunk.svelte";
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { parseMessageData } from "$lib/irc/protocol";
	import { vscodePath } from "$lib/project/project";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { getEditorUri, URL_SERVICE } from "$lib/utils/url";
	import { inject } from "@gitbutler/core/context";
	import {
		VirtualList,
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		Avatar,
		Markdown,
	} from "@gitbutler/ui";
	import type { StoredMessage } from "$lib/irc/ircApi";

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
		bannerText?: string;
		/** Return true if more history may exist, false when the top has been reached. */
		onLoadMore?: () => Promise<boolean>;
		onReply?: (msg: StoredMessage) => void;
	};

	const { projectId, messages, bannerText, onLoadMore, onReply }: Props = $props();

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
	const ircApiService = inject(IRC_API_SERVICE);
	const projectService = inject(PROJECTS_SERVICE);
	const urlService = inject(URL_SERVICE);

	const messageReactionsQuery = $derived(ircApiService.messageReactions());
	const messageReactions = $derived(messageReactionsQuery?.response ?? {});

	const nickQuery = $derived(ircApiService.nick());
	const myNick = $derived(nickQuery?.response);

	async function reactToMessage(msg: StoredMessage, emoji: string, remove = false) {
		if (!msg.msgid) return;
		const payload = JSON.stringify({
			type: "message-reaction",
			payload: { msgId: msg.msgid, reaction: emoji, ...(remove && { remove: true }) },
		});
		await ircApiService.sendMessageWithData({
			target: msg.target,
			message: remove ? `−${emoji}` : emoji,
			data: payload,
			replyTo: msg.msgid,
		});
	}

	function toggleReaction(msg: StoredMessage, emoji: string) {
		const reactions = msg.msgid ? (messageReactions[msg.msgid] ?? []) : [];
		const alreadyReacted =
			myNick && reactions.some((r) => r.sender === myNick && r.reaction === emoji);
		reactToMessage(msg, emoji, !!alreadyReacted);
	}

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let messagesEl = $state<HTMLDivElement>();

	let scrolling = $state(false);
	let scrollTimer: ReturnType<typeof setTimeout>;
	function onScroll() {
		scrolling = true;
		clearTimeout(scrollTimer);
		scrollTimer = setTimeout(() => {
			scrolling = false;
		}, 150);
	}

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
	<div class="message-row">
		{#if msg.msgid}
			<div class="message-actions">
				{#if onReply}
					<button type="button" class="action-btn" title="Reply" onclick={() => onReply(msg)}
						>↩</button
					>
				{/if}
				{#each ["👍", "❤️", "😂", "🎉"] as emoji}
					<button
						type="button"
						class="action-btn"
						title={emoji}
						onclick={() => toggleReaction(msg, emoji)}>{emoji}</button
					>
				{/each}
			</div>
		{/if}
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
					{projectId}
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
							onclick={() => toggleReaction(msg, group.emoji)}>{group.emoji} {group.count}</button
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
				{#if msg.tag}<span class="msg-tag">[{msg.tag}]</span>{/if}
				{msg.content}
				<span class="system-time">{timestamp}</span>
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

<div class="messages" class:scrolling bind:this={messagesEl} onscrollcapture={onScroll}>
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
</div>

<ContextMenu bind:this={contextMenu} rightClickTrigger={messagesEl} align="start" side="right">
	{#snippet children(item)}
		<ContextMenuSection>
			<ContextMenuItem
				label="Open in {$userSettings.defaultCodeEditor.displayName}"
				icon="open-in-ide"
				onclick={async () => {
					const project = await projectService.fetchProject(projectId);
					if (project?.path && item.filePath) {
						const path = getEditorUri({
							schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
							path: [vscodePath(project.path), item.filePath],
							line: item.lineNumber,
						});
						urlService.openExternalUrl(path);
					}
					contextMenu?.close();
				}}
			/>
		</ContextMenuSection>
	{/snippet}
</ContextMenu>

<style lang="postcss">
	.messages {
		position: relative;
		flex-grow: 1;
		overflow: hidden;
		background-color: var(--clr-bg-1);
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
	.messages:not(.scrolling) .message-row:hover {
		background-color: var(--clr-bg-2);
	}
	.message-actions {
		display: none;
		z-index: 1;
		position: absolute;
		right: 20px;
		bottom: calc(100% - 6px);
		gap: 2px;
		border: 1px solid var(--clr-bg-3);
		border-radius: 6px;
		background-color: var(--clr-bg-2);
	}
	.messages:not(.scrolling) .message-row:hover .message-actions {
		display: flex;
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
		color: var(--clr-text-3);
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
		gap: 8px;
	}
	.message-header {
		display: flex;
		align-items: baseline;
		gap: 6px;
	}
	.sender {
		color: var(--clr-text-1);
		font-weight: 600;
	}
	.timestamp {
		color: var(--clr-text-3);
	}
	.message-content {
		display: flex;
		flex-direction: column;
		gap: 8px;
		color: var(--clr-text-2);
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
			background-color: var(--clr-bg-2);
		}

		:global(.markdown code) {
			padding: 1px 4px;
			border-radius: var(--radius-s);
			background-color: var(--clr-bg-2);
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
		background-color: var(--clr-bg-2);
		font-size: 12px;
		cursor: pointer;
		&:hover {
			background-color: var(--clr-bg-3);
		}
		&.own {
			border-color: var(--clr-theme-pop-element);
			background-color: color-mix(in srgb, var(--clr-theme-pop-element) 15%, transparent);
		}
	}
	.reply-context {
		margin-bottom: 2px;
		padding-left: 6px;
		overflow: hidden;
		border-left: 2px solid var(--clr-border-2);
		color: var(--clr-text-3);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.system-message {
		padding: 0px 12px;
		color: var(--clr-text-2);
		line-height: 1;
		font-family: var(--font-mono);
		white-space: pre;
		user-select: text;
	}
	.system-time {
		color: var(--clr-text-3);
		opacity: 0.6;
	}
	.msg-tag {
		color: var(--clr-text-2);
		opacity: 0.7;
	}
	.channel-banner {
		padding: 16px 12px;
		color: var(--clr-text-3);
		text-align: center;
	}
</style>

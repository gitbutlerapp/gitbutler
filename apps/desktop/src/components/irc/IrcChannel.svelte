<script lang="ts">
	import IrcInput from "$components/irc/IrcInput.svelte";
	import IrcMessages from "$components/irc/IrcMessages.svelte";
	import IrcNames from "$components/irc/IrcNames.svelte";
	import { BACKEND } from "$lib/backend";
	import { getEditorUri, URL_SERVICE } from "$lib/backend/url";
	import { IRC_API_SERVICE, IRC_CONNECTION_ID } from "$lib/irc/ircApiService";
	import { vscodePath } from "$lib/project/project";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		ContextMenuItem,
		ContextMenuItemSubmenu,
		ContextMenuSection,
		KebabButton,
	} from "@gitbutler/ui";
	import { onDestroy } from "svelte";
	import type { StoredMessage } from "$lib/irc/ircEndpoints";
	import type { Snippet } from "svelte";

	type Props = {
		projectId: string;
		headerActions?: Snippet;
	} & (
		| { type: "server" }
		| { type: "group"; channel: string; autojoin: boolean }
		| { type: "private"; nick: string }
	);

	let props: Props = $props();

	const ircApiService = inject(IRC_API_SERVICE);
	const uiState = inject(UI_STATE);
	const backend = inject(BACKEND);
	const userSettings = inject(SETTINGS);
	const projectService = inject(PROJECTS_SERVICE);
	const urlService = inject(URL_SERVICE);

	// ── Queries & derived data ───────────────────────────────────────────

	const connectionStateQuery = $derived(ircApiService.connectionState());
	const isReady = $derived(connectionStateQuery?.response?.ready ?? false);

	const target = $derived.by(() => {
		switch (props.type) {
			case "group":
				return props.channel;
			case "private":
				return props.nick;
			case "server":
				return "*";
			default:
				return undefined;
		}
	});

	const messagesQuery = $derived(target ? ircApiService.messages({ channel: target }) : undefined);
	const messages = $derived(messagesQuery?.response ?? []);

	const nickQuery = $derived(ircApiService.nick());
	const myNick = $derived(nickQuery?.response);
	const messageReactionsQuery = $derived(ircApiService.messageReactions());
	const messageReactions = $derived(messageReactionsQuery?.response ?? {});

	const allUsersQuery = $derived(ircApiService.allUsers());
	const channelUsersQuery = $derived(
		props.type === "group" ? ircApiService.users({ channel: props.channel }) : undefined,
	);
	const channelUsers = $derived(channelUsersQuery?.response ?? []);
	const invitableUsers = $derived.by(() => {
		const all = allUsersQuery?.response ?? [];
		if (props.type !== "group") return [];
		const channelNicks = new Set(
			(channelUsersQuery?.response ?? []).map((u) =>
				/^[@+%~&]/.test(u.nick) ? u.nick.slice(1) : u.nick,
			),
		);
		return all.filter((nick) => !channelNicks.has(nick));
	});

	const editorName = $derived($userSettings.defaultCodeEditor.displayName);

	// ── Auto-join ────────────────────────────────────────────────────────

	$effect(() => {
		if (props.type === "group" && props.autojoin && isReady) {
			ircApiService.joinChannel({ channel: props.channel });
		}
	});

	// ── Mark read ────────────────────────────────────────────────────────
	// Debounce to avoid cascading refetches (markRead invalidates IrcChannels).

	let markReadTimer: ReturnType<typeof setTimeout> | undefined;
	let lastMarkedCount = 0;
	let lastMarkedTarget: string | undefined;

	$effect(() => {
		const count = messages.length;
		const ch = target;
		if (ch !== lastMarkedTarget) {
			lastMarkedCount = 0;
			lastMarkedTarget = ch;
		}
		if (count > 0 && ch && count !== lastMarkedCount) {
			clearTimeout(markReadTimer);
			markReadTimer = setTimeout(() => {
				lastMarkedCount = count;
				ircApiService.markRead({ channel: ch });
			}, 300);
		}
		return () => clearTimeout(markReadTimer);
	});

	// ── History loading ──────────────────────────────────────────────────

	let busy = false;
	let historyExhausted = false;

	$effect(() => {
		void target;
		historyExhausted = false;
	});

	async function handleLoadMore(): Promise<boolean> {
		if (busy || historyExhausted) return false;
		busy = true;
		try {
			if (!target || messages.length === 0) {
				return false;
			}
			const countBefore = messages.length;
			const oldest = messages[0]!;
			const before = new Date(oldest.timestamp).toISOString();
			await ircApiService.requestHistoryBefore({ channel: target, before });
			// Poll for RTKQ cache updates — check every 50ms for up to 5s.
			const hasMore = await new Promise<boolean>((resolve) => {
				let elapsed = 0;
				const interval = setInterval(() => {
					elapsed += 50;
					if (messages.length > countBefore) {
						clearInterval(interval);
						resolve(true);
					} else if (elapsed >= 5000) {
						clearInterval(interval);
						resolve(false);
					}
				}, 50);
			});
			if (!hasMore) historyExhausted = true;
			return hasMore;
		} finally {
			busy = false;
		}
	}

	// ── Reply state ──────────────────────────────────────────────────────

	let replyTo: StoredMessage | undefined = $state();

	function handleReply(msg: StoredMessage) {
		replyTo = msg;
	}

	function handleCancelReply() {
		replyTo = undefined;
	}

	// ── Typing indicators ────────────────────────────────────────────────

	const TYPING_TIMEOUT_MS = 6000;
	let typingMap = $state<Map<string, ReturnType<typeof setTimeout>>>(new Map());
	const typingUsers = $derived(Array.from(typingMap.keys()));

	const unlistenTyping = backend.listen<{ from: string; target: string; state: string }>(
		`irc:${IRC_CONNECTION_ID}:typing`,
		(event) => {
			const { from, target: evTarget, state: typingState } = event.payload;
			if (from === myNick || evTarget !== target) return;

			const existing = typingMap.get(from);
			if (existing) clearTimeout(existing);

			if (typingState === "done") {
				typingMap.delete(from);
				typingMap = new Map(typingMap);
			} else {
				const timer = setTimeout(() => {
					typingMap.delete(from);
					typingMap = new Map(typingMap);
				}, TYPING_TIMEOUT_MS);
				typingMap.set(from, timer);
				typingMap = new Map(typingMap);
			}
		},
	);

	// ── Actions ──────────────────────────────────────────────────────────

	function closeChannel() {
		if (props.type === "group") {
			ircApiService.partChannel({ channel: props.channel });
		}
		uiState.global.channel.set(undefined);
	}

	function inviteUser(nick: string) {
		if (props.type !== "group") return;
		ircApiService.sendRaw({ command: `INVITE ${nick} ${props.channel}` });
	}

	function startPrivateChat(nick: string) {
		uiState.global.channel.set(nick.replace(/^[@+%~&]/, ""));
	}

	async function toggleReaction(msg: StoredMessage, emoji: string) {
		if (!msg.msgid) return;
		const reactions = messageReactions[msg.msgid] ?? [];
		const alreadyReacted =
			myNick && reactions.some((r) => r.sender === myNick && r.reaction === emoji);
		if (alreadyReacted) {
			await ircApiService.removeReaction({ target: msg.target, msgid: msg.msgid, emoji });
		} else {
			await ircApiService.sendReaction({ target: msg.target, msgid: msg.msgid, emoji });
		}
	}

	async function deleteMessage(msg: StoredMessage) {
		if (!msg.msgid) return;
		await ircApiService.redactMessage({ target: msg.target, msgid: msg.msgid });
	}

	async function openInEditor(filePath: string, lineNumber?: number) {
		const project = await projectService.fetchProject(props.projectId);
		if (!project?.path) return;
		const uri = getEditorUri({
			schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
			path: [vscodePath(project.path), filePath],
			line: lineNumber,
		});
		urlService.openExternalUrl(uri);
	}

	// ── Cleanup ──────────────────────────────────────────────────────────

	onDestroy(() => {
		unlistenTyping();
		for (const timer of typingMap.values()) clearTimeout(timer);
	});
</script>

<div class="irc-channel">
	<div class="header text-14 text-semibold">
		<div class="header-left"></div>
		<div class="header-right" data-no-drag>
			{#if props.type !== "server"}
				<KebabButton>
					{#snippet contextMenu({ close })}
						<ContextMenuSection>
							{#if props.type === "group" && invitableUsers.length > 0}
								<ContextMenuItemSubmenu label="Invite user" icon="plus">
									{#snippet submenu({ close: closeSubmenu })}
										<ContextMenuSection>
											{#each invitableUsers as nick}
												<ContextMenuItem
													label={nick}
													onclick={() => {
														inviteUser(nick);
														closeSubmenu();
														close();
													}}
												/>
											{/each}
										</ContextMenuSection>
									{/snippet}
								</ContextMenuItemSubmenu>
							{/if}
							<ContextMenuItem
								label={props.type === "group" ? "Leave channel" : "Close chat"}
								icon="cross"
								onclick={() => {
									closeChannel();
									close();
								}}
							/>
						</ContextMenuSection>
					{/snippet}
				</KebabButton>
				<div class="divider"></div>
			{/if}

			{@render props.headerActions?.()}
		</div>
	</div>
	<div class="middle" data-no-drag>
		<div class="nested">
			{#key target}
				<IrcMessages
					projectId={props.projectId}
					{messages}
					{myNick}
					{messageReactions}
					{typingUsers}
					bannerText={props.type === "private"
						? `This is the beginning of your direct message history with ${props.nick}.`
						: props.type === "group"
							? `Welcome to ${props.channel}. This is the beginning of the channel.`
							: undefined}
					onLoadMore={handleLoadMore}
					onReply={handleReply}
					onToggleReaction={toggleReaction}
					onDeleteMessage={deleteMessage}
					{editorName}
					onOpenInEditor={openInEditor}
				/>
			{/key}
			{#if props.type === "group"}
				<IrcInput
					type="group"
					channel={props.channel}
					{replyTo}
					onCancelReply={handleCancelReply}
					onSent={handleCancelReply}
				/>
			{:else if props.type === "private"}
				<IrcInput
					type="private"
					nick={props.nick}
					{replyTo}
					onCancelReply={handleCancelReply}
					onSent={handleCancelReply}
				/>
			{:else if props.type === "server"}
				<IrcInput type="server" />
			{/if}
		</div>
		{#if props.type === "group"}
			<IrcNames users={channelUsers} onUserClick={startPrivateChat} />
		{/if}
	</div>
</div>

<style lang="postcss">
	.irc-channel {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		justify-content: space-between;
		width: 100%;
		height: 100%;
		background-color: var(--bg-1);
	}
	.header {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 100%;
		min-height: 32px;
		gap: 6px;
		background-color: var(--bg-mute);
	}
	.header-left {
		flex-grow: 1;
	}

	.header-right {
		display: flex;
		align-items: center;
		justify-content: right;
		padding-right: 6px;
		gap: 4px;
	}

	.middle {
		display: flex;
		flex-grow: 1;
		overflow: hidden;
		border-top: 1px solid var(--border-3);
	}

	.divider {
		width: 2px;
		height: 18px;
		background-color: var(--border-2);
	}

	.nested {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		overflow: hidden;
	}
</style>

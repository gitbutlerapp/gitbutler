<script lang="ts">
	import IrcInput from "$components/IrcInput.svelte";
	import IrcMessages from "$components/IrcMessages.svelte";
	import IrcNames from "$components/IrcNames.svelte";
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		ContextMenuItem,
		ContextMenuItemSubmenu,
		ContextMenuSection,
		KebabButton,
	} from "@gitbutler/ui";
	import type { StoredMessage } from "$lib/irc/ircApi";
	import type { Snippet } from "svelte";

	type Props = {
		projectId: string;
		type: string;
		headerElRef?: HTMLDivElement | undefined;
		headerActions?: Snippet;
	} & (
		| { type: "server" }
		| { type: "group"; channel: string; autojoin: boolean }
		| { type: "private"; nick: string }
	);

	let { headerElRef = $bindable(), ...props }: Props = $props();
	const ircApiService = inject(IRC_API_SERVICE);
	const uiState = inject(UI_STATE);

	const connectionStateQuery = $derived(ircApiService.connectionState());
	const isReady = $derived(connectionStateQuery?.response?.ready ?? false);

	// Auto-join channel when connection is ready
	$effect(() => {
		if (props.type === "group" && props.autojoin && isReady) {
			ircApiService.joinChannel({ channel: props.channel });
		}
	});

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

	const channelsQuery = $derived(ircApiService.channels());
	const topic = $derived.by(() => {
		if (props.type !== "group") return undefined;
		const ch = channelsQuery?.response?.find((c) => c.name === props.channel);
		return ch?.topic || undefined;
	});

	// Mark channel as read when viewing it and when new messages arrive.
	// Debounce to avoid cascading refetches (markRead invalidates IrcChannels).
	let markReadTimer: ReturnType<typeof setTimeout> | undefined;
	let lastMarkedCount = 0;
	$effect(() => {
		const count = messages.length;
		const ch = target;
		if (count > 0 && ch && count !== lastMarkedCount) {
			clearTimeout(markReadTimer);
			markReadTimer = setTimeout(() => {
				lastMarkedCount = count;
				ircApiService.markRead({ channel: ch });
			}, 300);
		}
	});

	function closeChannel() {
		switch (props.type) {
			case "group":
				ircApiService.partChannel({ channel: props.channel });
				break;
		}
		uiState.global.channel.set(undefined);
	}

	let replyTo: StoredMessage | undefined = $state();

	function handleReply(msg: StoredMessage) {
		replyTo = msg;
	}

	function handleCancelReply() {
		replyTo = undefined;
	}

	let busy = false;
	let historyExhausted = false;
	async function handleLoadMore(): Promise<boolean> {
		if (busy || historyExhausted) return false;
		busy = true;
		if (!target || messages.length === 0) {
			busy = false;
			return false;
		}
		const countBefore = messages.length;
		const oldest = messages[0]!;
		const before = new Date(oldest.timestamp).toISOString();
		await ircApiService.requestHistoryBefore({
			channel: target,
			before,
		});
		// Wait for the RTKQ cache to update with new messages
		return new Promise((resolve) => {
			setTimeout(() => {
				const hasMore = messages.length > countBefore;
				if (!hasMore) historyExhausted = true;
				busy = false;
				resolve(hasMore);
			}, 500);
		});
	}

	// Reset exhausted state when switching channels
	$effect(() => {
		target;
		historyExhausted = false;
	});

	// All known users across channels, minus users already in this channel.
	const allUsersQuery = $derived(ircApiService.allUsers());
	const channelUsersQuery = $derived(
		props.type === "group" ? ircApiService.users({ channel: props.channel }) : undefined,
	);
	const invitableUsers = $derived.by(() => {
		const all = allUsersQuery?.response ?? [];
		if (props.type !== "group") return [];
		const channelNicks = new Set(
			(channelUsersQuery?.response ?? []).map((u) =>
				u.nick.startsWith("@") ? u.nick.slice(1) : u.nick,
			),
		);
		return all.filter((nick) => !channelNicks.has(nick));
	});

	function inviteUser(nick: string) {
		if (props.type !== "group") return;
		ircApiService.sendRaw({ command: `INVITE ${nick} ${props.channel}` });
	}
</script>

<div class="irc-channel">
	<div class="header text-14 text-semibold" bind:this={headerElRef}>
		<div class="header-left"></div>
		<div class="header-center">
			{#if props.type === "group"}
				<span>{props.channel}</span>
				{#if topic}
					<span class="header-topic text-12">{topic}</span>
				{/if}
			{:else if props.type === "private"}
				{props.nick}
			{:else if props.type === "server"}
				server
			{/if}
		</div>
		<div class="header-right">
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
			{/if}
			{@render props.headerActions?.()}
		</div>
	</div>
	<div class="middle">
		<IrcMessages
			projectId={props.projectId}
			{messages}
			bannerText={props.type === "private"
				? `This is the beginning of your direct message history with ${props.nick}.`
				: props.type === "group"
					? `Welcome to ${props.channel}. This is the beginning of the channel.`
					: undefined}
			onLoadMore={handleLoadMore}
			onReply={handleReply}
		/>
		{#if props.type === "group"}
			<IrcNames channel={props.channel} />
		{/if}
	</div>
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

<style lang="postcss">
	.irc-channel {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		justify-content: space-between;
		width: 100%;
		height: 100%;
		background-color: var(--clr-bg-1);
	}
	.header {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 100%;
		min-height: 32px;
		gap: 6px;
		background-color: var(--clr-bg-muted);
	}
	.header-center {
		display: flex;
		flex-shrink: 1;
		flex-direction: column;
		align-items: center;
		padding: 4px 0;
		gap: 1px;
	}
	.header-topic {
		max-width: 400px;
		overflow: hidden;
		color: var(--clr-text-3);
		font-weight: normal;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.header-left {
		display: flex;
		gap: 14px;
	}

	.header-right {
		display: flex;
		justify-content: right;
		padding-right: 6px;
		gap: 4px;
	}

	.header-right,
	.header-left {
		flex-grow: 1;
		flex-basis: 0;
	}

	.middle {
		display: flex;
		flex-grow: 1;
		overflow: hidden;
		border-top: 1px solid var(--clr-border-3);
	}
</style>

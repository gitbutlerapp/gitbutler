<script lang="ts">
	import { IRC_SERVICE } from '$lib/irc/ircService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';

	const ircService = inject(IRC_SERVICE);
	const uiState = inject(UI_STATE);

	const selectedChannel = $derived(uiState.global.channel);
	const channels = $derived(ircService.getChannels());
	const chats = $derived(ircService.getChats());
</script>

<div class="channels text-13">
	<button type="button" class="channel server" onclick={() => selectedChannel.set(undefined)}>
		server
	</button>
	{#each channels.current as channel}
		{#if !channel?.floating}
			{@const unread = channel?.unread && channel.unread > 0}
			<button
				type="button"
				class="channel"
				class:unread
				class:selected={name === selectedChannel.current}
				onclick={() => selectedChannel.set(channel.name)}
			>
				{channel.name}
			</button>
		{/if}
	{/each}
	{#each chats.current as chat}
		{#if !chat?.floating}
			{@const nick = chat.username}
			{@const unread = chat?.unread && chat.unread > 0}
			<button
				type="button"
				class="nick"
				class:unread
				class:selected={nick === selectedChannel.current}
				onclick={() => selectedChannel.set(nick)}
			>
				{nick}
			</button>
		{/if}
	{/each}
</div>

<style lang="postcss">
	.channels {
		display: flex;
		flex-shrink: 0;
		flex-direction: column;
		width: 160px;
		padding: 12px 14px;
		overflow: hidden;
		border-right: 1px solid var(--clr-border-3);
	}
	.server {
		margin-bottom: 6px;
	}
	.channel,
	.nick {
		display: flex;
		padding: 4px 0;
		overflow: hidden;
		color: var(--clr-text-2);
		text-align: left;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.selected {
		color: var(--clr-text-1);
	}
	.unread {
		color: var(--clr-text-1);
		font-weight: 700;
	}
</style>

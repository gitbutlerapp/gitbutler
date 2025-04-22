<script lang="ts">
	import { IrcClient } from '$lib/irc/ircClient.svelte';
	import { IrcService } from '$lib/irc/ircService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';

	const [ircService, ircClient, uiState] = inject(IrcService, IrcClient, UiState);

	const selectedChannel = $derived(uiState.global.channel);
	const channels = $derived(ircService.getChannels());
	const chats = $derived(ircService.getChats());
</script>

<div class="channels text-13">
	<button type="button" class="channel server" onclick={() => selectedChannel.set(undefined)}>
		{ircClient.server}
	</button>
	{#each Object.keys(channels).sort() as name}
		{@const channel = channels[name]}
		{@const unread = channel?.unread && channel.unread > 0}
		<button
			type="button"
			class="channel"
			class:unread
			class:selected={name === selectedChannel.current}
			onclick={() => selectedChannel.set(name)}
		>
			{name}
		</button>
	{/each}
	{#each Object.keys(chats).sort() as nick}
		{@const chat = chats[nick]}
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
	{/each}
</div>

<style lang="postcss">
	.channels {
		display: flex;
		flex-shrink: 0;
		flex-direction: column;
		width: 160px;
		padding: 12px 14px;
		border-right: 1px solid var(--clr-border-3);
		overflow: hidden;
	}
	.server {
		margin-bottom: 6px;
	}
	.channel,
	.nick {
		display: flex;
		padding: 4px 0;
		text-align: left;
		text-overflow: ellipsis;
		white-space: nowrap;
		overflow: hidden;
		color: var(--clr-text-2);
	}

	.selected {
		color: var(--clr-text-1);
	}
	.unread {
		color: var(--clr-text-1);
		font-weight: 700;
	}
</style>

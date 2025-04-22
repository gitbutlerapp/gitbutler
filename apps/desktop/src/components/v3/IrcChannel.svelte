<script lang="ts">
	import IrcInput from '$components/v3/IrcInput.svelte';
	import IrcMessages from '$components/v3/IrcMessages.svelte';
	import IrcNames from '$components/v3/IrcNames.svelte';
	import { IrcClient } from '$lib/irc/ircClient.svelte';
	import { IrcService } from '$lib/irc/ircService.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		type: string;
	} & (
		| { type: 'server' }
		| { type: 'group'; channel: string; autojoin: boolean }
		| { type: 'private'; nick: string }
	);

	const props: Props = $props();
	const [ircService, ircClient] = inject(IrcService, IrcClient);

	$effect(() => {
		if (props.type === 'group' && props.autojoin && ircClient.connected) {
			ircService.send(`JOIN ${props.channel}`);
		}
	});

	$effect(() => {
		if (props.type === 'group') {
			return ircService.markOpen(props.channel);
		} else if (props.type === 'private') {
			return ircService.markOpen(props.nick);
		}
	});

	const logs = $derived.by(() => {
		switch (props.type) {
			case 'group':
				return ircService.getChannelMessages(props.channel);
			case 'private':
				return ircService.getPrivateMessages(props.nick);
			case 'server':
				return ircService.getServerMessages();
		}
	});
</script>

<div class="irc-channel">
	<div class="header text-14 text-semibold">
		{#if props.type === 'group'}
			{props.channel}
		{:else if props.type === 'private'}
			{props.nick}
		{:else if props.type === 'server'}
			system
		{/if}
	</div>
	<div class="middle">
		{#if logs}
			<IrcMessages {logs} />
		{/if}
		{#if props.type === 'group'}
			<IrcNames channel={props.channel} />
		{/if}
	</div>
	{#if props.type === 'group'}
		<IrcInput type="group" channel={props.channel} />
	{:else if props.type === 'private'}
		<IrcInput type="private" nick={props.nick} />
	{:else if props.type === 'server'}
		<IrcInput type="server" />
	{/if}
</div>

<style lang="postcss">
	.irc-channel {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		height: 100%;
		width: 100%;
		background-color: var(--clr-bg-1);
		border-radius: var(--radius-l);
	}
	.header {
		padding: 6px;
		width: 100%;
		text-align: center;
		background-color: var(--clr-bg-1);
		border-bottom: 1px solid var(--clr-border-2);
	}
	.middle {
		display: flex;
		overflow: hidden;
		flex-grow: 1;
	}
</style>

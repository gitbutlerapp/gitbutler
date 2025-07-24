<script lang="ts">
	import IrcInput from '$components/IrcInput.svelte';
	import IrcMessages from '$components/IrcMessages.svelte';
	import IrcNames from '$components/IrcNames.svelte';
	import { IRC_CLIENT } from '$lib/irc/ircClient.svelte';
	import { IRC_SERVICE } from '$lib/irc/ircService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { Snippet } from 'svelte';

	type Props = {
		type: string;
		headerElRef?: HTMLDivElement | undefined;
		headerActions?: Snippet;
	} & (
		| { type: 'server' }
		| { type: 'group'; channel: string; autojoin: boolean }
		| { type: 'private'; nick: string }
	);

	let { headerElRef = $bindable(), ...props }: Props = $props();
	const ircService = inject(IRC_SERVICE);
	const ircClient = inject(IRC_CLIENT);

	$effect(() => {
		if (props.type === 'group' && props.autojoin && ircClient.connected) {
			ircService.send(`JOIN ${props.channel}`);
		}
	});

	let i = 0;

	$effect(() => {
		if (i++ > 2) {
			return;
		}
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
	<div class="header text-14 text-semibold" bind:this={headerElRef}>
		<div class="header-left"></div>
		<div class="header-center">
			{#if props.type === 'group'}
				{props.channel}
			{:else if props.type === 'private'}
				{props.nick}
			{:else if props.type === 'server'}
				{ircClient.server.current}
			{/if}
		</div>
		<div class="header-right">
			{@render props.headerActions?.()}
		</div>
	</div>
	<div class="middle">
		<IrcMessages {logs} />
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
		justify-content: space-between;
		width: 100%;
		height: 100%;
		background-color: var(--clr-bg-1);
	}
	.header {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		padding: 6px;
		gap: 6px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}
	.header-center {
		flex-shrink: 1;
	}

	.header-left {
		display: flex;
		gap: 14px;
	}

	.header-right {
		display: flex;
		justify-content: right;
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
	}
</style>

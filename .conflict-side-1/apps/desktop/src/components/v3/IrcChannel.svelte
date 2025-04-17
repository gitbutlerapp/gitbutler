<script lang="ts">
	import IrcInput from '$components/v3/IrcInput.svelte';
	import IrcMessages from '$components/v3/IrcMessages.svelte';
	import IrcNames from '$components/v3/IrcNames.svelte';
	import { IrcService } from '$lib/irc/ircService.svelte';
	import { getContext } from '@gitbutler/shared/context';

	type Props = {
		type: string;
	} & (
		| { type: 'system' }
		| { type: 'group'; channel: string; autojoin: boolean }
		| { type: 'private'; nick: string }
	);

	const props: Props = $props();
	const ircService = getContext(IrcService);

	$effect(() => {
		if (props.type === 'group' && props.autojoin) {
			ircService.send(`JOIN ${props.channel}`);
		}
	});

	$effect(() => {
		if (props.type === 'group') {
			return ircService.markOpen(props.channel);
		}
	});

	const logs = $derived(
		props.type === 'group'
			? ircService.getChannelMessages(props.channel)
			: ircService.getSystemMessages()
	);

	const channelName = $derived(props.type === 'group' ? props.channel : undefined);
</script>

<div class="irc-channel">
	<div class="header text-14 text-semibold">
		{#if props.type === 'group'}
			{props.channel}
		{:else if props.type === 'system'}
			system
		{:else if props.type === 'private'}
			private
		{/if}
	</div>
	<div class="middle">
		{#if logs}
			<IrcMessages channel={channelName} {logs} />
		{/if}
		{#if props.type === 'group'}
			<IrcNames channel={props.channel} />
		{/if}
	</div>
	<IrcInput channel={props.type === 'group' ? props.channel : undefined} />
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

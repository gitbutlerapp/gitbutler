<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import IrcInput from '$components/v3/IrcInput.svelte';
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

	const logs = $derived(
		props.type === 'group'
			? ircService.getChannelMessages(props.channel)
			: ircService.getSystemMessages()
	);
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
	<div class="messages">
		<ConfigurableScrollableContainer>
			{#each logs || [] as log}
				<div class="message" class:error={log.type === 'outgoing' && log.error}>
					{#if log.type === 'incoming'}
						[{new Date(log.timestamp).toLocaleTimeString()}] {log.from}: {log.message}
					{:else if log.type === 'outgoing'}
						[{new Date(log.timestamp).toLocaleTimeString()}] {log.from}: {log.message}
					{:else if log.type === 'server'}
						[{new Date(log.timestamp).toLocaleTimeString()}] {log.message}
					{:else if log.type === 'command'}
						[{new Date(log.timestamp).toLocaleTimeString()}] {log.raw}
					{/if}
				</div>
				{#if log.type === 'outgoing' && log.error}
					{log.error}
				{/if}
			{/each}
		</ConfigurableScrollableContainer>
	</div>
	<IrcInput channel={props.type === 'group' ? props.channel : undefined} />
</div>

<style lang="postcss">
	.irc-channel {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		height: 100%;
		background-color: var(--clr-bg-1);
		border-radius: var(--radius-l);
	}
	.header {
		padding: 6px;
		width: 100%;
		text-align: center;
		background-color: var(--clr-bg-2);
		border-bottom: 1px solid var(--clr-border-2);
	}
	.messages {
		flex-grow: 1;
		overflow: hidden;
		padding: 6px;
	}
	.message {
		font-family: monospace;
		white-space: pre-wrap;
	}
	.error {
		background-color: var(--clr-scale-err-90);
	}
</style>

<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import type { IrcLog } from '$lib/irc/types';

	type Props = {
		channel?: string;
		logs: IrcLog[];
	};

	const { channel, logs }: Props = $props();

	let scroller: ConfigurableScrollableContainer;

	$effect(() => {
		if (channel) scroller.scrollToBottom();
	});
</script>

<div class="messages">
	<ConfigurableScrollableContainer bind:this={scroller}>
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

<style lang="postcss">
	.messages {
		flex-grow: 1;
		overflow: hidden;
	}
	.message {
		font-family: var(--fontfamily-mono);
		white-space: pre-wrap;
		user-select: text;
	}
	.error {
		background-color: var(--clr-scale-err-90);
	}
</style>

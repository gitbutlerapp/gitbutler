<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import type { IrcLog } from '$lib/irc/types';

	type Props = {
		logs: IrcLog[];
	};

	const { logs }: Props = $props();
</script>

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

<style lang="postcss">
	.message {
		font-family: monospace;
		white-space: pre-wrap;
		user-select: text;
	}
	.error {
		background-color: var(--clr-scale-err-90);
	}
</style>

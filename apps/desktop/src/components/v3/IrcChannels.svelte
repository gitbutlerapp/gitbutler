<script lang="ts">
	import { IrcClient } from '$lib/irc/ircClient.svelte';
	import { IrcService } from '$lib/irc/ircService.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		onselect: (channel?: string) => void;
	};

	const { onselect }: Props = $props();

	const [ircService, ircClient] = inject(IrcService, IrcClient);

	const channels = $derived(ircService.getChannels());
</script>

<div class="channels text-13">
	<button
		type="button"
		class="channel"
		onclick={() => {
			onselect(undefined);
		}}
	>
		{ircClient.server}
	</button>
	{#each Object.keys(channels) as name}
		{@const channel = channels[name]}
		<button
			type="button"
			class="channel"
			class:text-bold={channel?.unread && channel.unread > 0}
			onclick={() => {
				onselect(name);
			}}
		>
			{name}
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
	.channel {
		display: flex;
		padding: 4px 0;
		text-align: left;
		text-overflow: ellipsis;
		white-space: nowrap;
		overflow: hidden;
	}
</style>

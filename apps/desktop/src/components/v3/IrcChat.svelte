<script lang="ts">
	import IrcChannel from '$components/v3/IrcChannel.svelte';
	import IrcChannels from '$components/v3/IrcChannels.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';

	const uiState = getContext(UiState);
	const channel = $derived(uiState.global.channel);
</script>

<div class="irc">
	<IrcChannels />
	<div class="right">
		{#if channel.current?.startsWith('#')}
			<IrcChannel type="group" channel={channel.current} autojoin />
		{:else if channel.current}
			<IrcChannel type="private" nick={channel.current} />
		{:else}
			<IrcChannel type="server" />
		{/if}
	</div>
</div>

<style lang="postcss">
	.irc {
		display: flex;
		width: 100%;
		height: 100%;
		overflow: hidden;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
	}
	.right {
		flex-grow: 1;
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}
</style>

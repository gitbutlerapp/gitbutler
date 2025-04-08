<script lang="ts">
	import IrcChannel from '$components/v3/IrcChannel.svelte';
	import IrcChannels from '$components/v3/IrcChannels.svelte';

	let channel: string | undefined = $state();
</script>

<div class="irc">
	<IrcChannels
		onselect={(name) => {
			channel = name;
		}}
	/>
	<div class="right">
		{#if channel}
			<IrcChannel type="group" {channel} autojoin />
		{:else}
			<IrcChannel type="system" />
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

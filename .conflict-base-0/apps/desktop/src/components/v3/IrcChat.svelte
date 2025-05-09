<script lang="ts">
	import IrcChannel from '$components/v3/IrcChannel.svelte';
	import IrcChannels from '$components/v3/IrcChannels.svelte';
	import { IrcService } from '$lib/irc/ircService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Toggle from '@gitbutler/ui/Toggle.svelte';

	const [ircService, uiState] = inject(IrcService, UiState);
	const currentName = $derived(uiState.global.channel.current);
</script>

<div class="irc">
	<IrcChannels />
	<div class="right">
		{#if currentName}
			{#if currentName.startsWith('#')}
				<IrcChannel type="group" channel={currentName} autojoin />
			{:else}
				{@const chat = ircService.getChat(currentName)}
				{#if chat.current}
					<IrcChannel type="private" nick={chat.current.username}>
						{#snippet headerActions()}
							<Toggle
								small
								checked={chat.current?.popup}
								onchange={(checked) => {
									ircService.setPopup(currentName, checked);
								}}
							/>
						{/snippet}
					</IrcChannel>
				{/if}
			{/if}
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

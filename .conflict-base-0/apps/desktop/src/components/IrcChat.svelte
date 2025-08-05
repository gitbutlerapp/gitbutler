<script lang="ts">
	import IrcChannel from '$components/IrcChannel.svelte';
	import IrcChannels from '$components/IrcChannels.svelte';
	import { IRC_SERVICE } from '$lib/irc/ircService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { Button } from '@gitbutler/ui';

	const ircService = inject(IRC_SERVICE);
	const uiState = inject(UI_STATE);
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
							<Button
								icon="open-link"
								size="icon"
								kind="ghost"
								onclick={() => {
									ircService.setPopup(currentName, true);
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
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
	}
	.right {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}
</style>

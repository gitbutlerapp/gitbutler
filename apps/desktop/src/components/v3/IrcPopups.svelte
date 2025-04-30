<script lang="ts">
	import IrcChannel from '$components/v3/IrcChannel.svelte';
	import { IrcService } from '$lib/irc/ircService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	const [ircService] = inject(IrcService);

	const chats = $derived(ircService.getChatsWithPopup());
</script>

<div class="irc-popups">
	{#if chats.length > 0}
		<div class="irc-popup">
			{#each chats as chat}
				<div class="popup-content">
					<IrcChannel nick={chat.username} type="private">
						{#snippet headerActions()}
							<Button
								size="icon"
								style="ghost"
								icon="arrow-bottom"
								onclick={() => {
									ircService.setPopup(chat.username, false);
								}}
							/>
						{/snippet}
					</IrcChannel>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style lang="postcss">
	.irc-popups {
		position: absolute;
		right: 6px;
		bottom: 0;
	}
	.irc-popup {
		display: flex;
		flex-direction: column;
		width: 360px;
		border-color: var(--clr-border-2);
		border-width: 1px 1px 0 1px;
		border-style: solid;
	}
	.popup-content {
		height: 300px;
	}
</style>

<script lang="ts">
	import IrcChannel from '$components/v3/IrcChannel.svelte';
	import IrcFloat from '$components/v3/IrcFloat.svelte';
	import { IrcService } from '$lib/irc/ircService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	const [ircService] = inject(IrcService);

	const chats = $derived(ircService.getChatsWithPopup());
	let collapsed = $state(false);
</script>

{#if chats.current.length > 0}
	{#each chats.current as chat}
		<IrcFloat
			persistId={chat.username}
			initialPosition={{ x: 100, y: 100 }}
			initialSize={{ width: 260, height: 320 }}
		>
			<IrcChannel nick={chat.username} type="private">
				{#snippet headerActions()}
					<Button
						size="icon"
						icon="cross"
						kind="ghost"
						onclick={() => {
							ircService.setPopup(chat.username, false);
						}}
					/>
					<Button
						size="icon"
						icon={collapsed ? 'chevron-down-small' : 'chevron-up-small'}
						kind="ghost"
						onclick={() => {
							collapsed = !collapsed;
						}}
					/>
				{/snippet}
			</IrcChannel>
		</IrcFloat>
	{/each}
{/if}

<style lang="postcss">
</style>

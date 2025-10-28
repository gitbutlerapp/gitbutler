<script lang="ts">
	import IrcChannel from '$components/IrcChannel.svelte';
	import FloatingModal from '$lib/floating/FloatingModal.svelte';
	import { IRC_SERVICE } from '$lib/irc/ircService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Button } from '@gitbutler/ui';

	const ircService = inject(IRC_SERVICE);

	const chats = $derived(ircService.getChatsWithPopup());
	let ircHeaderEl: HTMLDivElement | undefined = $state();
	let collapsed = $state(false);
</script>

{#if chats.current.length > 0}
	{#each chats.current as chat}
		<FloatingModal
			defaults={{
				width: 260,
				minWidth: 260,
				height: 320,
				minHeight: 320,
				snapPosition: 'top-right'
			}}
			dragHandleElement={ircHeaderEl}
		>
			<IrcChannel bind:headerElRef={ircHeaderEl} nick={chat.username} type="private">
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
		</FloatingModal>
	{/each}
{/if}

<style lang="postcss">
</style>

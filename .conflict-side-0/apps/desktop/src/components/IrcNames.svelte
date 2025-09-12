<script lang="ts">
	import { IRC_SERVICE } from '$lib/irc/ircService.svelte';
	import { inject } from '@gitbutler/core/context';

	type Props = {
		channel: string;
	};

	const { channel }: Props = $props();

	const ircService = inject(IRC_SERVICE);

	const users = $derived(ircService.getChannelUsers(channel));
</script>

<div class="channel-users text-13">
	{#if users.current}
		{#each Object.keys(users.current).sort() as name}
			{@const user = users.current[name]}
			<button
				type="button"
				class="user"
				onclick={() => {
					// onselect(name);
				}}
			>
				{user?.nick}
			</button>
		{/each}
	{/if}
</div>

<style lang="postcss">
	.channel-users {
		display: flex;
		flex-shrink: 0;
		flex-direction: column;
		width: fit-content;
		padding: 12px 14px;
		overflow: hidden;
		border-left: 1px solid var(--clr-border-3);
	}
	.user {
		display: flex;
		padding: 4px 0;
		overflow: hidden;
		text-align: left;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>

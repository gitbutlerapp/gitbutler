<script lang="ts">
	import { IrcService } from '$lib/irc/ircService.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		channel: string;
	};

	const { channel }: Props = $props();

	const [ircService] = inject(IrcService);

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
		border-left: 1px solid var(--clr-border-3);
		overflow: hidden;
	}
	.user {
		display: flex;
		padding: 4px 0;
		text-align: left;
		text-overflow: ellipsis;
		white-space: nowrap;
		overflow: hidden;
	}
</style>

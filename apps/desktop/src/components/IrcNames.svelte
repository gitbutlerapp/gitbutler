<script lang="ts">
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";

	type Props = {
		channel: string;
	};

	const { channel }: Props = $props();

	const ircApiService = inject(IRC_API_SERVICE);
	const uiState = inject(UI_STATE);

	const usersQuery = $derived(ircApiService.users({ channel }));
	const users = $derived(usersQuery.response ?? []);
	const sortedUsers = $derived([...users].sort((a, b) => a.nick.localeCompare(b.nick)));

	function startPrivateChat(nick: string) {
		uiState.global.channel.set(nick);
	}
</script>

<div class="channel-users text-13">
	{#each sortedUsers as user}
		<button
			type="button"
			class="user"
			class:away={user.away}
			onclick={() => startPrivateChat(user.nick)}
		>
			{user.nick}
		</button>
	{/each}
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
		background-color: var(--clr-bg-muted);
	}
	.user {
		display: flex;
		align-items: center;
		padding: 4px 0;
		overflow: hidden;
		gap: 6px;
		text-align: left;
		text-overflow: ellipsis;
		white-space: nowrap;
		cursor: pointer;
		transition: color var(--transition-fast);
	}
	.user.away {
		color: var(--clr-text-3);
	}
</style>

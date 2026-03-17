<script lang="ts">
	import type { UserEntry } from "$lib/irc/ircApi";

	type Props = {
		users: UserEntry[];
		onUserClick?: (nick: string) => void;
	};

	const { users, onUserClick }: Props = $props();

	const sortedUsers = $derived([...users].sort((a, b) => a.nick.localeCompare(b.nick)));
</script>

<div class="channel-users text-13">
	{#each sortedUsers as user}
		<button
			type="button"
			class="user"
			class:away={user.away}
			onclick={() => onUserClick?.(user.nick)}
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

<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { UserService } from '@gitbutler/shared/users/userService';
	import { getUserByLogin } from '@gitbutler/shared/users/usersPreview.svelte';
	interface Props {
		username: string;
	}

	const { username }: Props = $props();

	const appState = getContext(AppState);
	const userService = getContext(UserService);

	const user = $derived(getUserByLogin(appState, userService, username));
</script>

<Loading loadable={user.current}>
	{#snippet children(user)}
		<div class="mention-suggestion">
			<div class="mention-suggestion__header">
				{#if user}
					<img src={user?.avatarUrl} alt={username} class="mention-suggestion__avatar" />
				{/if}
				<p class="text-13 text-semibold truncate">
					@{username}
				</p>
			</div>

			{#if user}
				<div class="mention-suggestion__body">
					<p class="mention-suggestion__name text-12 text-tertiary truncate">
						{user?.name}
					</p>
				</div>
			{/if}
		</div>
	{/snippet}
</Loading>

<style>
	.mention-suggestion {
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.mention-suggestion__header {
		flex-grow: 1;
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.mention-suggestion__avatar {
		width: 16px;
		height: 16px;
		border-radius: 50%;
	}

	.mention-suggestion__name {
		opacity: 0.4;
	}
</style>

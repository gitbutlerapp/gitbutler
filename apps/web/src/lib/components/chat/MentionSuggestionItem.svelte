<script lang="ts">
	import { inject } from '@gitbutler/core/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { APP_STATE } from '@gitbutler/shared/redux/store.svelte';
	import { USER_SERVICE } from '@gitbutler/shared/users/userService';
	import { getUserByLogin } from '@gitbutler/shared/users/usersPreview.svelte';
	interface Props {
		username: string;
	}

	const { username }: Props = $props();

	const appState = inject(APP_STATE);
	const userService = inject(USER_SERVICE);

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
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
	}

	.mention-suggestion__header {
		display: flex;
		flex-grow: 1;
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

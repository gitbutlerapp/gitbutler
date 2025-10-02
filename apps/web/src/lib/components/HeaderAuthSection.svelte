<script lang="ts">
	import { goto } from '$app/navigation';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';

	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, Icon, Tooltip } from '@gitbutler/ui';

	const userService = inject(USER_SERVICE);
	const user = $derived(userService.user);
	const routes = inject(WEB_ROUTES_SERVICE);
</script>

<div class="header-auth-section">
	{#if $user}
		<Tooltip text="Profile & Settings">
			<button
				type="button"
				class="user-btn"
				onclick={() => {
					goto(routes.profilePath());
				}}
			>
				{#if $user?.picture}
					<img class="user-icon__image" src={$user.picture} alt="" referrerpolicy="no-referrer" />
				{:else}
					<Icon name="profile" />
				{/if}
			</button>
		</Tooltip>
	{:else}
		<div class="login-signup-wrap">
			<Button kind="outline" onclick={() => goto(routes.signupPath())}>Join GitButler</Button>
			<Button style="pop" onclick={() => goto(routes.loginPath())} icon="signin">Log in</Button>
		</div>
	{/if}
</div>

<style lang="postcss">
	.header-auth-section {
		display: flex;
		align-items: center;
		gap: 12px;

		@media (--tablet-viewport) {
			gap: 10px;
		}
	}

	.user-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-button);
		height: var(--size-button);
		overflow: hidden;
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-pop-element);
		color: var(--clr-core-ntrl-100);

		& img {
			width: 100%;
			height: 100%;
			object-fit: cover;
		}
	}

	.login-signup-wrap {
		display: flex;
		align-items: center;
		gap: 8px;
	}
</style>

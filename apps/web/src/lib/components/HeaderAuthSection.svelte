<script lang="ts">
	import { goto } from '$app/navigation';
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import { featureShowOrganizations } from '$lib/featureFlags';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, ContextMenu, ContextMenuItem, ContextMenuSection, Icon } from '@gitbutler/ui';
	import { env } from '$env/dynamic/public';

	const authService = inject(AUTH_SERVICE);
	const token = $derived(authService.tokenReadable);

	const userService = inject(USER_SERVICE);
	const user = $derived(userService.user);
	const routes = inject(WEB_ROUTES_SERVICE);

	let ctxMenuUserEl = $state<ReturnType<typeof ContextMenu>>();
	let ctxUserTriggerButton = $state<HTMLButtonElement | undefined>();
	let isCtxMenuOpen = $state(false);

	function logout() {
		authService.clearToken();
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/logout`;
	}
</script>

<div class="header-auth-section">
	{#if $user}
		<Button
			kind="outline"
			class="user-btn"
			activated={isCtxMenuOpen}
			onclick={() => {
				ctxMenuUserEl?.toggle();
			}}
			bind:el={ctxUserTriggerButton}
		>
			<div class="user-btn">
				<div class="user-icon">
					{#if $user?.picture}
						<img class="user-icon__image" src={$user.picture} alt="" referrerpolicy="no-referrer" />
					{:else}
						<Icon name="profile" />
					{/if}
				</div>
				<div class="user-select-icon">
					<Icon name="select-chevron" />
				</div>
			</div>
		</Button>
	{:else}
		<div class="login-signup-wrap">
			<Button kind="outline" onclick={() => goto(routes.signupPath())}>Join GitButler</Button>
			<Button style="pop" onclick={() => goto(routes.loginPath())} icon="signin">Log in</Button>
		</div>
	{/if}
</div>

<ContextMenu
	bind:this={ctxMenuUserEl}
	leftClickTrigger={ctxUserTriggerButton}
	side="right"
	align="end"
	ontoggle={(isOpen) => (isCtxMenuOpen = isOpen)}
>
	<ContextMenuSection>
		<ContextMenuItem
			label="Preferences"
			onclick={() => {
				goto('/profile');
			}}
		/>
		{#if $token && $featureShowOrganizations}
			<ContextMenuItem label="Organizations" onclick={() => goto('/organizations')} />
		{/if}
	</ContextMenuSection>
	<ContextMenuSection>
		<ContextMenuItem label="Log out" onclick={logout} />
	</ContextMenuSection>
</ContextMenu>

<style lang="postcss">
	.header-auth-section {
		display: flex;
		align-items: center;
		gap: 12px;

		@media (--tablet-viewport) {
			gap: 10px;
		}
	}

	/* override profile button */
	:global(.header-auth-section .user-btn) {
		padding: 0;
	}

	:global(.header-auth-section .user-btn .label) {
		padding: 0 3px;
	}

	.user-btn {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.user-select-icon {
		display: flex;
		margin-right: 2px;
		color: var(--label-clr);
		opacity: var(--icon-opacity);
		transition:
			opacity var(--transition-fast),
			stroke var(--transition-fast);
	}

	.user-icon {
		width: 20px;
		height: 20px;
		overflow: hidden;
		border-radius: var(--radius-s);
	}

	/* login */
	.login-signup-wrap {
		display: flex;
		align-items: center;
		gap: 8px;
	}
</style>

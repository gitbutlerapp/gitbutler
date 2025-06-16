<script lang="ts">
	import { goto } from '$app/navigation';
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import Breadcrumbs from '$lib/components/chat/Breadcrumbs.svelte';
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

<nav class="navigation">
	<div class="main-links">
		<a href={routes.homePath()} class="logo" aria-label="main nav" title="Home">
			<svg
				width="23"
				height="22"
				viewBox="0 0 23 22"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path d="M0 22V0L11.4819 9.63333L23 0V22L11.4819 12.4L0 22Z" fill="var(--clr-text-1)" />
			</svg>
		</a>

		<Breadcrumbs />
	</div>

	<div class="other-links">
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
							<img
								class="user-icon__image"
								src={$user.picture}
								alt=""
								referrerpolicy="no-referrer"
							/>
						{:else}
							<Icon name="profile" />
						{/if}
					</div>
					<div class="user-select-icon">
						<Icon name="select-chevron" />
					</div>
				</div></Button
			>
		{:else}
			<div class="login-signup-wrap">
				<a href={routes.signupPath()}>
					<Button kind="outline">Join GitButler</Button>
				</a>
				<a href={routes.loginPath()} title="Log in" aria-label="Log in">
					<Button style="pop" icon="signin">Log in</Button>
				</a>
			</div>
		{/if}
	</div>
</nav>

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
	<ContextMenuSection title="Theme (⌘K)">
		<ContextMenuItem
			label="Dark"
			onclick={async () => {
				// TODO: implement theme switching
			}}
		/>
		<ContextMenuItem
			label="Light"
			onclick={async () => {
				// TODO: implement theme switching
			}}
		/>
		<ContextMenuItem
			label="System"
			onclick={async () => {
				// TODO: implement theme switching
			}}
		/>
	</ContextMenuSection>

	<ContextMenuSection>
		<ContextMenuItem label="Log out" onclick={logout} />
	</ContextMenuSection>
</ContextMenu>

<style lang="postcss">
	.navigation {
		display: flex;
		justify-content: space-between;
		width: 100%;
		padding: 0 0 24px;
		gap: 16px;
	}

	.main-links {
		display: flex;
		flex: 1;
		align-items: center;
		overflow: hidden;
		gap: 16px;
	}

	.logo {
		display: flex;
	}

	.other-links {
		display: flex;
		align-items: center;
		gap: 12px;

		@media (--tablet-viewport) {
			gap: 10px;
		}
	}

	/* override profile button */
	:global(.navigation .user-btn) {
		padding: 0;
	}

	:global(.navigation .user-btn .label) {
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

	/* MODIFIERS */
	:global(.navigation .hidden-on-desktop) {
		display: none;

		@media (--mobile-viewport) {
			display: block;
		}
	}
</style>

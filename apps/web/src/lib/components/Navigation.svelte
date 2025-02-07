<script lang="ts">
	import { AuthService } from '$lib/auth/authService.svelte';
	import Breadcrumbs from '$lib/components/breadcrumbs/Breadcrumbs.svelte';
	import { featureShowOrganizations } from '$lib/featureFlags';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import NotificationButton from '@gitbutler/ui/NotificationButton.svelte';
	import { goto } from '$app/navigation';
	import { env } from '$env/dynamic/public';

	const authService = getContext(AuthService);
	const token = $derived(authService.tokenReadable);

	const userService = getContext(UserService);
	const user = $derived(userService.user);

	let ctxMenuUserEl = $state<ReturnType<typeof ContextMenu>>();
	let ctxUserTriggerButton = $state<HTMLButtonElement | undefined>();
	let isCtxMenuOpen = $state(false);

	let isNotificationsUnread = $state(false);

	function login() {
		window.location.href = `${env.PUBLIC_APP_HOST}/cloud/login?callback=${window.location.href}`;
	}
	function logout() {
		authService.clearToken();
		window.location.href = `${env.PUBLIC_APP_HOST}/cloud/logout`;
	}
</script>

<div class="navigation">
	<div class="main-links">
		<a href="/repositories" class="logo" aria-label="main nav" title="Home">
			<svg
				width="23"
				height="22"
				viewBox="0 0 23 22"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path d="M0 22V0L11.4819 9.63333L23 0V22L11.4819 12.4L0 22Z" fill="#1A1614" />
			</svg>
		</a>

		<Breadcrumbs />
	</div>

	<div class="other-links">
		{#if $user}
			<NotificationButton
				hasUnread={isNotificationsUnread}
				onclick={() => {
					// TODO: implement notifications
					console.log('Example of the button animation');
					isNotificationsUnread = !isNotificationsUnread;
				}}
			/>

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
			<Button kind="outline" icon="signin" onclick={login}>Log in / Sign up</Button>
		{/if}
	</div>
</div>

<ContextMenu
	bind:this={ctxMenuUserEl}
	leftClickTrigger={ctxUserTriggerButton}
	side="right"
	verticalAlign="bottom"
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
		<ContextMenuItem label="Logout" onclick={logout} />
	</ContextMenuSection>
</ContextMenu>

<style lang="postcss">
	.navigation {
		display: flex;
		justify-content: space-between;
		width: 100%;
		padding: 0 0 24px;
	}

	.main-links {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 16px;
		overflow: hidden;
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
		border-radius: var(--radius-ml);
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
		color: var(--label-clr);
		opacity: var(--icon-opacity);
		margin-right: 2px;
		transition:
			opacity var(--transition-fast),
			stroke var(--transition-fast);
	}

	.user-icon {
		width: 20px;
		height: 20px;
		border-radius: var(--radius-m);
		overflow: hidden;
	}

	:global(.navigation .hidden-on-desktop) {
		display: none;

		@media (--mobile-viewport) {
			display: block;
		}
	}
</style>

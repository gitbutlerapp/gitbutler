<script lang="ts">
	import { AuthService } from '$lib/auth/authService.svelte';
	import Breadcrumbs from '$lib/components/breadcrumbs/Breadcrumbs.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import NotificationButton from '@gitbutler/ui/NotificationButton.svelte';
	import { goto } from '$app/navigation';
	import { env } from '$env/dynamic/public';

	const routes = getContext(WebRoutesService);

	const authService = getContext(AuthService);
	const token = $derived(authService.tokenReadable);

	const userService = getContext(UserService);
	const user = $derived(userService.user);

	let ctxMenuUserEl = $state<ReturnType<typeof ContextMenu>>();
	let ctxUserTriggerButton = $state<HTMLButtonElement | undefined>();
	let isCtxMenuOpen = $state(false);

	let ctxMenuOtherLinks = $state<ReturnType<typeof ContextMenu>>();
	let ctxOtherLinksTriggerButton = $state<HTMLButtonElement | undefined>();
	let isCtxOtherLinksMenuOpen = $state(false);

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
		<a href="/" class="logo" aria-label="main nav" title="Home">
			<svg xmlns="http://www.w3.org/2000/svg" width="23" height="24" viewBox="0 0 23 24">
				<path d="M0 24V0L11.4819 10.5091L23 0V24L11.4819 13.5273L0 24Z" />
			</svg>
		</a>
		<Breadcrumbs />
	</div>

	<div class="other-links">
		{#if $token}
			<a
				class="text-12 text-semibold other-nav-link"
				href={routes.projectsPath()}
				aria-label="projects">Projects</a
			>
			<a
				class="text-12 text-semibold other-nav-link"
				href="/organizations"
				aria-label="organizations"
			>
				Organizations
			</a>

			<Button
				kind="ghost"
				icon="kebab"
				class="hidden-on-desktop"
				bind:el={ctxOtherLinksTriggerButton}
				activated={isCtxOtherLinksMenuOpen}
				onclick={() => {
					ctxMenuOtherLinks?.toggle();
				}}
			/>
		{/if}

		{#if !$user}
			<a
				class="text-12 text-semibold other-nav-link"
				href="/downloads"
				aria-label="downloads"
				title="Downloads"
			>
				Downloads
			</a>
		{/if}

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
	bind:this={ctxMenuOtherLinks}
	leftClickTrigger={ctxOtherLinksTriggerButton}
	side="right"
	verticalAlign="bottom"
	ontoggle={(isOpen) => (isCtxOtherLinksMenuOpen = isOpen)}
>
	<ContextMenuSection>
		<ContextMenuItem label="Projects" onclick={() => goto(routes.projectsPath())} />
		<ContextMenuItem label="Organizations" onclick={() => goto('/organizations')} />
	</ContextMenuSection>
</ContextMenu>

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
				goto('/settings/profile');
			}}
		/>
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
		<ContextMenuItem label="Downloads" onclick={() => goto('/downloads')} />
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
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.logo {
		display: flex;
	}

	.other-links {
		display: flex;
		align-items: center;
		gap: 20px;

		@media (--tablet-viewport) {
			gap: 10px;
		}
	}

	.other-nav-link {
		color: var(--clr-text-2);
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-1);
			text-decoration: underline;
		}

		@media (--tablet-viewport) {
			display: none;
		}
	}

	/* override profile button */
	:global(.navigation .user-btn) {
		border-radius: var(--radius-ml);
		padding: 0;
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

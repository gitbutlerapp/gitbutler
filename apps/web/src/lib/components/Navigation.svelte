<script lang="ts">
	import { AuthService } from '$lib/auth/authService';
	import Breadcrumbs from '$lib/components/breadcrumbs/Breadcrumbs.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { goto } from '$app/navigation';
	import { env } from '$env/dynamic/public';

	const routes = getContext(WebRoutesService);

	const authService = getContext(AuthService);
	const token = $derived(authService.token);

	const userService = getContext(UserService);
	const user = $derived(userService.user);

	let contextMenuEl = $state<ReturnType<typeof ContextMenu>>();
	let contextUserTriggerButton = $state<HTMLButtonElement | undefined>();
	let isContextMenuOpen = $state(false);

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
			<Button
				kind="outline"
				class="user-btn"
				activated={isContextMenuOpen}
				onclick={() => {
					contextMenuEl?.toggle();
				}}
				bind:el={contextUserTriggerButton}
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
	bind:this={contextMenuEl}
	leftClickTrigger={contextUserTriggerButton}
	side="right"
	verticalAlign="bottom"
	ontoggle={(isOpen) => (isContextMenuOpen = isOpen)}
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

<style>
	.navigation {
		display: flex;
		justify-content: space-between;
		width: 100%;
		padding: 24px 0;
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
		gap: 16px;
	}

	.other-nav-link {
		color: var(--clr-text-2);
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
</style>

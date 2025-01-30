<script lang="ts">
	import '$lib/styles/global.css';
	import { AuthService } from '$lib/auth/authService';
	import Navigation from '$lib/components/Navigation.svelte';
	import { UserService } from '$lib/user/userService';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import { HttpClient } from '@gitbutler/shared/network/httpClient';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { UserService as NewUserService } from '@gitbutler/shared/users/userService';
	import { setExternalLinkService } from '@gitbutler/ui/link/externalLinkService';
	import { setContext, type Snippet } from 'svelte';
	import { get } from 'svelte/store';
	import { Toaster } from 'svelte-french-toast';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { env } from '$env/dynamic/public';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const authService = new AuthService();
	setContext(AuthService, authService);
	let token = $derived(authService.token);

	const httpClient = new HttpClient(window.fetch, env.PUBLIC_APP_HOST, authService.token);
	setContext(HttpClient, httpClient);

	const userService = new UserService(httpClient);
	setContext(UserService, userService);

	const appState = new AppState();
	setContext(AppState, appState);
	const feedService = new FeedService(httpClient, appState.appDispatch);
	setContext(FeedService, feedService);
	const organizationService = new OrganizationService(httpClient, appState.appDispatch);
	setContext(OrganizationService, organizationService);
	const projectService = new ProjectService(httpClient, appState.appDispatch);
	setContext(ProjectService, projectService);
	const newUserService = new NewUserService(httpClient, appState.appDispatch);
	setContext(NewUserService, newUserService);
	const branchService = new BranchService(httpClient, appState.appDispatch);
	setContext(BranchService, branchService);
	const patchSerice = new PatchService(httpClient, appState.appDispatch);
	setContext(PatchService, patchSerice);
	const chatChannelService = new ChatChannelsService(httpClient, appState.appDispatch);
	setContext(ChatChannelsService, chatChannelService);
	const repositoryIdLookupService = new RepositoryIdLookupService(httpClient, appState.appDispatch);
	setContext(RepositoryIdLookupService, repositoryIdLookupService);
	const latestBranchLookupService = new LatestBranchLookupService(httpClient, appState.appDispatch);
	setContext(LatestBranchLookupService, latestBranchLookupService);
	const routesService = new WebRoutesService(location.protocol + '//' + location.host, true);
	setContext(WebRoutesService, routesService);
	setExternalLinkService({
		open: (href) => {
			location.href = href;
		}
	});

	$effect(() => {
		const token = get(authService.token) || $page.url.searchParams.get('gb_access_token');
		if (token) {
			authService.setToken(token);

			if ($page.url.searchParams.has('gb_access_token')) {
				$page.url.searchParams.delete('gb_access_token');
				goto(`?${$page.url.searchParams.toString()}`);
			}
		}
	});
</script>

<Toaster />

{#if (!$token && $page.url.pathname === '/') || $page.url.pathname === '/home'}
	<section class="page-wrapper">
		{@render children()}
	</section>
{:else}
	<div class="app">
		<Navigation />
		<main>
			{@render children()}
		</main>
	</div>
{/if}

<style>
	.app {
		display: flex;
		flex-direction: column;
		min-height: 100vh;
		max-width: 1440px;
		padding: 0 80px;
		margin: 0 auto;
	}

	main {
		flex: 1;
		display: flex;
		flex-direction: column;
		margin: 0 auto;
		width: 100%;
	}

	.page-wrapper {
		display: flex;
		flex-direction: column;
		max-width: 1280px;
		margin: 0 auto;

		@media (max-width: 1280px) {
			padding: 0 24px;
		}
	}
</style>

<script lang="ts">
	import '$lib/styles/global.css';
	import { AuthService } from '$lib/auth/authService';
	import Navigation from '$lib/components/Navigation.svelte';
	import LoggedOutNavigation from '$lib/components/LoggedOutNavigation.svelte';
	import { UserService } from '$lib/user/userService';
	import {
		CloudRepositoriesService,
		RepositoriesApiService
	} from '@gitbutler/shared/cloud/repositories/service';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import { HttpClient } from '@gitbutler/shared/httpClient';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { WebRoutesService, setRoutesService } from '@gitbutler/shared/sharedRoutes';
	import { UserService as NewUserService } from '@gitbutler/shared/users/userService';
	import { setContext, type Snippet } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { env } from '$env/dynamic/public';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const webRoutesService = new WebRoutesService();
	setRoutesService(webRoutesService);

	const authService = new AuthService();
	setContext(AuthService, authService);
	const token = $derived(authService.token);

	const httpClient = new HttpClient(window.fetch, env.PUBLIC_APP_HOST, authService.token);
	setContext(HttpClient, httpClient);

	const userService = new UserService(httpClient);
	setContext(UserService, userService);

	const repositoriesApiService = new RepositoriesApiService(httpClient);
	const cloudRepositoriesService = new CloudRepositoriesService(repositoriesApiService);
	setContext(CloudRepositoriesService, cloudRepositoriesService);

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


	$effect(() => {
		if ($page.url.searchParams.get('gb_access_token')) {
			const token = $page.url.searchParams.get('gb_access_token');
			if (token && token.length > 0) {
				authService.setToken(token);

				$page.url.searchParams.delete('gb_access_token');
				goto(`?${$page.url.searchParams.toString()}`);
			}
		}
	});
</script>

{#if $token}
<div class="app">
	<div class="sidebar-nav">
		<Navigation />
	</div>
	<main class="main-w-nav">
		{@render children()}
	</main>
</div>
{:else}
	<LoggedOutNavigation />
	<main>
		{@render children()}
	</main>
{/if}

<style>
	.sidebar-nav {
		position: fixed;
		top: 0;
		left: 0;
		bottom: 0;
		width: 36px;
		max-width: 36px;
		background-color: var(--color-background);
	}

	.app {
		display: flex;
		flex-direction: column;
		min-height: 100vh;
		width: 90%;
	}

	main {
		flex: 1;
		display: flex;
		flex-direction: column;
		padding: 20px;
		margin: 0 auto;
		width: 100%;
	}
	.main-w-nav {
		margin-left: 64px;
	}
</style>

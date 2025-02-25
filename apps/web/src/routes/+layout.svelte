<script lang="ts">
	import Header from '$home/lib/components/Header.svelte';
	import BlogHighlights from '$home/lib/sections/BlogHighlights.svelte';
	import DevelopersReview from '$home/lib/sections/DevelopersReview.svelte';
	import FAQ from '$home/lib/sections/FAQ.svelte';
	import Features from '$home/lib/sections/Features.svelte';
	import HomeFooter from '$home/lib/sections/Footer.svelte';
	import Hero from '$home/lib/sections/Hero.svelte';
	import { AuthService } from '$lib/auth/authService.svelte';
	import Footer from '$lib/components/Footer.svelte';
	import Navigation from '$lib/components/Navigation.svelte';
	import { UserService } from '$lib/user/userService';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { PatchEventsService } from '@gitbutler/shared/branches/patchEventsService';
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import { HttpClient } from '@gitbutler/shared/network/httpClient';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { NotificationSettingsService } from '@gitbutler/shared/settings/notificationSettingsService';
	import { UserService as NewUserService } from '@gitbutler/shared/users/userService';
	import { setExternalLinkService } from '@gitbutler/ui/link/externalLinkService';
	import { setContext, type Snippet } from 'svelte';
	import { get } from 'svelte/store';
	import { Toaster } from 'svelte-french-toast';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import '$lib/styles/global.css';
	import '$home/styles/styles.css';
	import { env } from '$env/dynamic/public';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const authService = new AuthService();
	setContext(AuthService, authService);
	let token = $state<string | null>();

	// Parse searchParams for token
	const searchParams = $derived(page.url.searchParams);

	$effect(() => {
		token = get(authService.tokenReadable) || searchParams.get('gb_access_token');
		if (token) {
			authService.setToken(token);

			if (page.url.searchParams.has('gb_access_token')) {
				page.url.searchParams.delete('gb_access_token');
				goto(`?${page.url.searchParams.toString()}`);
			}
		}
	});

	$effect(() => {
		if (!token) {
			goto('/');
		}
	});

	const httpClient = new HttpClient(window.fetch, env.PUBLIC_APP_HOST, authService.tokenReadable);
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
	const patchEventsService = new PatchEventsService(
		httpClient,
		appState,
		appState.appDispatch,
		authService.tokenReadable,
		patchSerice,
		env.PUBLIC_APP_HOST
	);
	setContext(PatchEventsService, patchEventsService);
	const chatChannelService = new ChatChannelsService(httpClient, appState.appDispatch);
	setContext(ChatChannelsService, chatChannelService);
	const repositoryIdLookupService = new RepositoryIdLookupService(httpClient, appState.appDispatch);
	setContext(RepositoryIdLookupService, repositoryIdLookupService);
	const latestBranchLookupService = new LatestBranchLookupService(httpClient, appState.appDispatch);
	setContext(LatestBranchLookupService, latestBranchLookupService);
	const routesService = new WebRoutesService(location.protocol + '//' + location.host, true);
	setContext(WebRoutesService, routesService);
	const notificationSettingsService = new NotificationSettingsService(
		httpClient,
		appState.appDispatch
	);
	setContext(NotificationSettingsService, notificationSettingsService);
	setExternalLinkService({
		open: (href) => {
			location.href = href;
		}
	});
</script>

<Toaster />

{#if token}
	<div class="app">
		<Navigation />
		<main>
			{@render children?.()}
		</main>
		<Footer />
	</div>
{:else}
	<section class="page-wrapper">
		<Header />
		<Hero />
		<Features />
		<DevelopersReview />
		<BlogHighlights />
		<FAQ />
		<HomeFooter />
	</section>
{/if}

<style lang="postcss">
	.app {
		--layout-side-paddings: 80px;

		display: flex;
		flex-direction: column;
		min-height: 100vh;
		max-width: calc(1440px + var(--layout-side-paddings) * 2);
		padding: 24px var(--layout-side-paddings);
		margin: 0 auto;

		@media (--tablet-viewport) {
			--layout-side-paddings: 40px;
			padding: 24px var(--layout-side-paddings);
		}

		@media (--mobile-viewport) {
			--layout-side-paddings: 16px;
			padding: var(--layout-side-paddings);
		}
	}

	main {
		flex: 1;
		display: flex;
		flex-direction: column;
		margin: 0 auto;
		width: 100%;
	}

	:global(.page-wrapper) {
		display: flex;
		flex-direction: column;
		max-width: 1280px;
		margin: 0 auto;
	}
</style>

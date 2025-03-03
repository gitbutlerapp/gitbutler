<script lang="ts">
	import { AuthService } from '$lib/auth/authService.svelte';
	import Footer from '$lib/components/Footer.svelte';
	import Navigation from '$lib/components/Navigation.svelte';
	import { UserService } from '$lib/user/userService';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
	import { getContext } from '@gitbutler/shared/context';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import { HttpClient } from '@gitbutler/shared/network/httpClient';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { PatchEventsService } from '@gitbutler/shared/patchEvents/patchEventsService';
	import { PatchCommitService } from '@gitbutler/shared/patches/patchCommitService';
	import { PatchIdableService } from '@gitbutler/shared/patches/patchIdableService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { NotificationSettingsService } from '@gitbutler/shared/settings/notificationSettingsService';
	import { UserService as NewUserService } from '@gitbutler/shared/users/userService';
	import { setExternalLinkService } from '@gitbutler/ui/link/externalLinkService';
	import { setContext, type Snippet } from 'svelte';
	import { Toaster } from 'svelte-french-toast';
	import '$lib/styles/global.css';
	import { page } from '$app/state';
	import { env } from '$env/dynamic/public';

	const CHAT_NOTFICATION_SOUND = '/sounds/pop.mp3';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const authService = getContext(AuthService);

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
	const patchService = new PatchCommitService(httpClient, appState.appDispatch);
	setContext(PatchCommitService, patchService);
	const patchEventsService = new PatchEventsService(
		httpClient,
		appState,
		appState.appDispatch,
		authService.tokenReadable,
		patchService,
		env.PUBLIC_APP_HOST
	);
	const patchIdableService = new PatchIdableService(httpClient, appState.appDispatch);
	setContext(PatchIdableService, patchIdableService);

	const user = $derived(userService.user);

	$effect(() => {
		if ($user) {
			patchEventsService.setUserId($user.id);
			patchEventsService.setChatSoundUrl(CHAT_NOTFICATION_SOUND);
		}
	});

	setContext(PatchEventsService, patchEventsService);
	const chatChannelService = new ChatChannelsService(httpClient, appState.appDispatch);
	setContext(ChatChannelsService, chatChannelService);
	const repositoryIdLookupService = new RepositoryIdLookupService(httpClient, appState.appDispatch);
	setContext(RepositoryIdLookupService, repositoryIdLookupService);
	const latestBranchLookupService = new LatestBranchLookupService(httpClient, appState.appDispatch);
	setContext(LatestBranchLookupService, latestBranchLookupService);
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

	const isCommitPage = $derived(page.url.pathname.includes('/commit/'));
</script>

<Toaster />

<div class="app">
	{#if !isCommitPage}
		<Navigation />
	{/if}

	<main>
		{@render children?.()}
	</main>
	<Footer />
</div>

<style lang="postcss">
	.app {
		--layout-side-paddings: 80px;

		display: flex;
		flex-direction: column;
		min-height: 100vh;
		max-width: calc(1440px + var(--layout-side-paddings) * 2);
		padding: 24px var(--layout-side-paddings);
		margin: 0 auto;
		container-type: inline-size;

		@media (--desktop-small-viewport) {
			--layout-side-paddings: 40px;
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
</style>

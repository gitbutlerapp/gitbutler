<script lang="ts">
	import '$lib/styles/global.css';
	import { page } from '$app/state';
	import { ButlerAIClient } from '$lib/ai/service';
	import { AuthService } from '$lib/auth/authService.svelte';
	import Footer from '$lib/components/Footer.svelte';
	import Navigation from '$lib/components/Navigation.svelte';
	import { OwnerService } from '$lib/owner/ownerService';
	import { WebState } from '$lib/redux/store.svelte';
	import { SshKeyService } from '$lib/sshKeyService';
	import { UserService } from '$lib/user/userService';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
	import { getContext } from '@gitbutler/shared/context';
	import { FeedService } from '@gitbutler/shared/feeds/service';
	import LoginService from '@gitbutler/shared/login/loginService';
	import { HttpClient } from '@gitbutler/shared/network/httpClient';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { PatchEventsService } from '@gitbutler/shared/patchEvents/patchEventsService';
	import { PatchCommitService } from '@gitbutler/shared/patches/patchCommitService';
	import { PatchIdableService } from '@gitbutler/shared/patches/patchIdableService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { RulesService } from '@gitbutler/shared/rules/rulesService';
	import { NotificationSettingsService } from '@gitbutler/shared/settings/notificationSettingsService';
	import { UploadsService } from '@gitbutler/shared/uploads/uploadsService';
	import { UserService as NewUserService } from '@gitbutler/shared/users/userService';
	import { setExternalLinkService } from '@gitbutler/ui/link/externalLinkService';
	import { setContext, type Snippet } from 'svelte';
	import { Toaster } from 'svelte-french-toast';
	import { env } from '$env/dynamic/public';

	const CHAT_NOTFICATION_SOUND = '/sounds/pop.mp3';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const authService = getContext(AuthService);

	const httpClient = new HttpClient(window.fetch, env.PUBLIC_APP_HOST, authService.tokenReadable);
	setContext(HttpClient, httpClient);

	const loginService = new LoginService(httpClient);
	setContext(LoginService, loginService);

	const aiService = new ButlerAIClient(httpClient);
	setContext(ButlerAIClient, aiService);

	const userService = new UserService(httpClient);
	setContext(UserService, userService);

	const webState = new WebState();
	setContext(AppState, webState);
	setContext(WebState, webState);
	const feedService = new FeedService(httpClient, webState.appDispatch);
	setContext(FeedService, feedService);
	const organizationService = new OrganizationService(httpClient, webState.appDispatch);
	setContext(OrganizationService, organizationService);
	const projectService = new ProjectService(httpClient, webState.appDispatch);
	setContext(ProjectService, projectService);
	const newUserService = new NewUserService(httpClient, webState.appDispatch);
	setContext(NewUserService, newUserService);
	const branchService = new BranchService(httpClient, webState.appDispatch);
	setContext(BranchService, branchService);
	const patchService = new PatchCommitService(httpClient, webState.appDispatch);
	setContext(PatchCommitService, patchService);
	const patchEventsService = new PatchEventsService(
		httpClient,
		webState,
		webState.appDispatch,
		authService.tokenReadable,
		patchService,
		env.PUBLIC_APP_HOST
	);
	const patchIdableService = new PatchIdableService(httpClient, webState.appDispatch);
	setContext(PatchIdableService, patchIdableService);

	const user = $derived(userService.user);

	$effect(() => {
		if ($user) {
			patchEventsService.setUserId($user.id);
			patchEventsService.setChatSoundUrl(CHAT_NOTFICATION_SOUND);
		}
	});

	setContext(PatchEventsService, patchEventsService);
	const chatChannelService = new ChatChannelsService(httpClient, webState.appDispatch);
	setContext(ChatChannelsService, chatChannelService);
	const repositoryIdLookupService = new RepositoryIdLookupService(httpClient, webState.appDispatch);
	setContext(RepositoryIdLookupService, repositoryIdLookupService);
	const latestBranchLookupService = new LatestBranchLookupService(httpClient, webState.appDispatch);
	setContext(LatestBranchLookupService, latestBranchLookupService);
	const notificationSettingsService = new NotificationSettingsService(
		httpClient,
		webState.appDispatch
	);
	setContext(NotificationSettingsService, notificationSettingsService);
	setExternalLinkService({
		open: (href) => {
			location.href = href;
		}
	});

	const sshKeyService = new SshKeyService(httpClient);
	setContext(SshKeyService, sshKeyService);
	const uploadsService = new UploadsService(httpClient);
	setContext(UploadsService, uploadsService);

	const ownerService = new OwnerService(httpClient);
	setContext(OwnerService, ownerService);

	const rulesService = new RulesService(httpClient, webState.appDispatch);
	setContext(RulesService, rulesService);

	const isCommitPage = $derived(page.url.pathname.includes('/commit/'));
	const isLoginPage = $derived(page.url.pathname.includes('/login'));
	const hasNavigation = $derived(!isCommitPage && !isLoginPage);
</script>

<Toaster />

<div class="app">
	{#if hasNavigation}
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
		container-type: inline-size;

		display: flex;
		flex-direction: column;
		max-width: calc(1440px + var(--layout-side-paddings) * 2);
		min-height: 100vh;
		margin: 0 auto;
		padding: 24px var(--layout-side-paddings);

		@media (--desktop-small-viewport) {
			--layout-side-paddings: 40px;
		}

		@media (--mobile-viewport) {
			--layout-side-paddings: 16px;
			padding: var(--layout-side-paddings);
		}
	}

	main {
		display: flex;
		flex: 1;
		flex-direction: column;
		width: 100%;
		margin: 0 auto;
	}
</style>

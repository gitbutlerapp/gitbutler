<script lang="ts">
	import '../../styles/global.css';
	import { page } from '$app/state';
	import { ButlerAIClient, BUTLER_AI_CLIENT } from '$lib/ai/service';
	import RedirectIfNotFinalized from '$lib/auth/RedirectIfNotFinalized.svelte';
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import CompactFooter from '$lib/components/CompactFooter.svelte';
	import Navigation from '$lib/components/Navigation.svelte';
	import { OwnerService, OWNER_SERVICE } from '$lib/owner/ownerService';
	import { WebState, WEB_STATE } from '$lib/redux/store.svelte';
	import { SshKeyService, SSH_KEY_SERVICE } from '$lib/sshKeyService';
	import { UserService, USER_SERVICE } from '$lib/user/userService';
	import { inject, provide } from '@gitbutler/core/context';
	import { BranchService, BRANCH_SERVICE } from '@gitbutler/shared/branches/branchService';
	import {
		LatestBranchLookupService,
		LATEST_BRANCH_LOOKUP_SERVICE
	} from '@gitbutler/shared/branches/latestBranchLookupService';
	import {
		ChatChannelsService,
		CHAT_CHANNELS_SERVICE
	} from '@gitbutler/shared/chat/chatChannelsService';
	import { FeedService, FEED_SERVICE } from '@gitbutler/shared/feeds/service';
	import LoginService, { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { HttpClient, HTTP_CLIENT } from '@gitbutler/shared/network/httpClient';
	import {
		OrganizationService,
		ORGANIZATION_SERVICE
	} from '@gitbutler/shared/organizations/organizationService';
	import { ProjectService, PROJECT_SERVICE } from '@gitbutler/shared/organizations/projectService';
	import {
		RepositoryIdLookupService,
		REPOSITORY_ID_LOOKUP_SERVICE
	} from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import {
		PatchEventsService,
		PATCH_EVENTS_SERVICE
	} from '@gitbutler/shared/patchEvents/patchEventsService';
	import {
		PatchCommitService,
		PATCH_COMMIT_SERVICE
	} from '@gitbutler/shared/patches/patchCommitService';
	import {
		PatchIdableService,
		PATCH_IDABLE_SERVICE
	} from '@gitbutler/shared/patches/patchIdableService';
	import { APP_STATE } from '@gitbutler/shared/redux/store.svelte';
	import { RulesService, RULES_SERVICE } from '@gitbutler/shared/rules/rulesService';
	import {
		NotificationSettingsService,
		NOTIFICATION_SETTINGS_SERVICE
	} from '@gitbutler/shared/settings/notificationSettingsService';
	import { UploadsService, UPLOADS_SERVICE } from '@gitbutler/shared/uploads/uploadsService';
	import {
		UserService as NewUserService,
		USER_SERVICE as NEW_USER_SERVICE
	} from '@gitbutler/shared/users/userService';
	import { ChipToastContainer } from '@gitbutler/ui';
	import {
		EXTERNAL_LINK_SERVICE,
		type ExternalLinkService
	} from '@gitbutler/ui/utils/externalLinkService';
	import { type Snippet } from 'svelte';
	import { env } from '$env/dynamic/public';

	const CHAT_NOTFICATION_SOUND = '/sounds/pop.mp3';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const authService = inject(AUTH_SERVICE);

	const httpClient = new HttpClient(window.fetch, env.PUBLIC_APP_HOST, authService.tokenReadable);
	provide(HTTP_CLIENT, httpClient);

	const loginService = new LoginService(httpClient);
	provide(LOGIN_SERVICE, loginService);

	const aiService = new ButlerAIClient(httpClient);
	provide(BUTLER_AI_CLIENT, aiService);

	const userService = new UserService(httpClient);
	provide(USER_SERVICE, userService);

	const webState = new WebState();
	provide(APP_STATE, webState);
	provide(WEB_STATE, webState);
	const feedService = new FeedService(httpClient, webState.appDispatch);
	provide(FEED_SERVICE, feedService);
	const organizationService = new OrganizationService(httpClient, webState.appDispatch);
	provide(ORGANIZATION_SERVICE, organizationService);
	const projectService = new ProjectService(httpClient, webState.appDispatch);
	provide(PROJECT_SERVICE, projectService);
	const newUserService = new NewUserService(httpClient, webState.appDispatch);
	provide(NEW_USER_SERVICE, newUserService);
	const branchService = new BranchService(httpClient, webState.appDispatch);
	provide(BRANCH_SERVICE, branchService);
	const patchService = new PatchCommitService(httpClient, webState.appDispatch);
	provide(PATCH_COMMIT_SERVICE, patchService);
	const patchEventsService = new PatchEventsService(
		httpClient,
		webState,
		webState.appDispatch,
		authService.tokenReadable,
		patchService,
		env.PUBLIC_APP_HOST
	);
	const patchIdableService = new PatchIdableService(httpClient, webState.appDispatch);
	provide(PATCH_IDABLE_SERVICE, patchIdableService);

	const user = $derived(userService.user);

	$effect(() => {
		if ($user) {
			patchEventsService.setUserId($user.id);
			patchEventsService.setChatSoundUrl(CHAT_NOTFICATION_SOUND);
		}
	});

	provide(PATCH_EVENTS_SERVICE, patchEventsService);
	const chatChannelService = new ChatChannelsService(httpClient, webState.appDispatch);
	provide(CHAT_CHANNELS_SERVICE, chatChannelService);
	const repositoryIdLookupService = new RepositoryIdLookupService(httpClient, webState.appDispatch);
	provide(REPOSITORY_ID_LOOKUP_SERVICE, repositoryIdLookupService);
	const latestBranchLookupService = new LatestBranchLookupService(httpClient, webState.appDispatch);
	provide(LATEST_BRANCH_LOOKUP_SERVICE, latestBranchLookupService);
	const notificationSettingsService = new NotificationSettingsService(
		httpClient,
		webState.appDispatch
	);
	provide(NOTIFICATION_SETTINGS_SERVICE, notificationSettingsService);
	provide(EXTERNAL_LINK_SERVICE, {
		open: (href) => {
			location.href = href;
		}
	} satisfies ExternalLinkService);

	const sshKeyService = new SshKeyService(httpClient);
	provide(SSH_KEY_SERVICE, sshKeyService);
	const uploadsService = new UploadsService(httpClient);
	provide(UPLOADS_SERVICE, uploadsService);

	const ownerService = new OwnerService(httpClient);
	provide(OWNER_SERVICE, ownerService);

	const rulesService = new RulesService(httpClient, webState.appDispatch);
	provide(RULES_SERVICE, rulesService);

	const isCommitPage = $derived(page.url.pathname.includes('/commit/'));
	const isLoginPage = $derived(page.url.pathname.includes('/login'));
	const isSignupPage = $derived(page.url.pathname.includes('/signup'));
	const isFinalized = $derived(page.url.pathname.includes('/finalize'));
	const isLoggedinPage = $derived(page.url.pathname === '/loggedin');
	const hasNavigation = $derived(
		!isCommitPage && !isLoginPage && !isSignupPage && !isFinalized && !isLoggedinPage
	);
	const fillFullWidth = $derived(isLoginPage || isSignupPage || isFinalized);
</script>

<RedirectIfNotFinalized />

<div class="app" class:fill-full-width={fillFullWidth}>
	<Navigation markOnly={!hasNavigation} />

	<main>
		{@render children?.()}
	</main>
	<CompactFooter />
</div>

<ChipToastContainer />

<style lang="postcss">
	.app {
		--radius-xl: 20px;
		container-type: inline-size;
		display: flex;
		flex-direction: column;
		width: 100%;
		min-height: 100vh;
		margin: 0 auto;
		padding: 24px var(--layout-side-paddings) 30px;

		&:not(.fill-full-width) {
			max-width: calc(1440px + var(--layout-side-paddings) * 2);
		}

		@media (--mobile-viewport) {
			padding: var(--layout-side-paddings);
		}
	}

	main {
		display: flex;
		flex: 1;
		flex-direction: column;
		width: 100%;
		min-height: 100%;
		margin: 0 auto;
	}
</style>

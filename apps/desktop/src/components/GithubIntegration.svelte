<script lang="ts">
	import GithubUserLoginState from '$components/GithubUserLoginState.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { OnboardingEvent, POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import githubLogoSvg from '$lib/assets/unsized-logos/github.svg?raw';
	import { CLIPBOARD_SERVICE } from '$lib/backend/clipboard';
	import { GITHUB_USER_SERVICE } from '$lib/forge/github/githubUserService.svelte';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { URL_SERVICE } from '$lib/utils/url';
	import { inject } from '@gitbutler/core/context';

	import {
		Button,
		CardGroup,
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		Textbox,
		chipToasts as toasts
	} from '@gitbutler/ui';
	import { fade } from 'svelte/transition';

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;
	const githubUserService = inject(GITHUB_USER_SERVICE);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const posthog = inject(POSTHOG_WRAPPER);

	const [clearAll, clearingAllResult] = githubUserService.deleteAllGitHubAccounts();
	const [storePat, storePatResult] = githubUserService.storeGitHubPat;
	const [storeGhePat, storeGhePatResult] = githubUserService.storeGithuibEnterprisePat;
	const accounts = githubUserService.accounts();

	let showingFlow = $state<'oauthFlow' | 'pat' | 'ghe'>();

	// OAuth step flags
	let codeCopied = $state(false);
	let GhActivationLinkPressed = $state(false);
	let GhActivationPageOpened = $state(false);

	let loading = $state(false);
	let userCode = $state('');
	let deviceCode = $state('');

	// PAT flow state
	let patInput = $state<string>();
	let patError = $state<string>();

	// GitHub Enterprise flow state
	let ghePatInput = $state<string>();
	let gheHostInput = $state<string>();
	let ghePatError = $state<string>();
	let gheHostError = $state<string>();

	// Add account button and context menu
	let addProfileButtonRef = $state<HTMLElement>();
	let addAccountContextMenu = $state<ContextMenu>();

	function cleanupAuthFlow() {
		showingFlow = undefined;
		codeCopied = false;
		GhActivationLinkPressed = false;
		GhActivationPageOpened = false;
	}

	function cleanupPatFlow() {
		showingFlow = undefined;
		patInput = undefined;
		patError = undefined;
	}

	function cleanupGheFlow() {
		showingFlow = undefined;
		ghePatInput = undefined;
		gheHostInput = undefined;
		ghePatError = undefined;
		gheHostError = undefined;
	}

	function gitHubStartOauth() {
		posthog.captureOnboarding(OnboardingEvent.GitHubInitiateOAuth);
		githubUserService.initDeviceOauth().then((verification) => {
			userCode = verification.user_code;
			deviceCode = verification.device_code;
			showingFlow = 'oauthFlow';
			// Reset all step flags for a fresh auth flow
			codeCopied = false;
			GhActivationLinkPressed = false;
			GhActivationPageOpened = false;
		});
	}

	async function gitHubOauthCheckStatus(deviceCode: string) {
		loading = true;
		try {
			await githubUserService.checkAuthStatus({ deviceCode });
			toasts.success($t('settings.general.integrations.github.authenticated'));
		} catch (err: any) {
			console.error(err);
			toasts.error($t('settings.general.integrations.github.authFailed'));
			posthog.captureOnboarding(OnboardingEvent.GitHubOAuthFailed);
		} finally {
			// Reset the auth flow on completion
			cleanupAuthFlow();
			loading = false;
		}
	}

	async function deleteAllGitHubAccounts() {
		await clearAll();
		gitHubStartOauth();
	}

	function startPatFlow() {
		showingFlow = 'pat';
	}
	async function storePersonalAccessToken() {
		if (!patInput) return;
		patError = undefined;
		try {
			await storePat({ accessToken: patInput });
			posthog.captureOnboarding(OnboardingEvent.GitHubStorePat);
			cleanupPatFlow();
		} catch (err: any) {
			console.error('Failed to store GitHub PAT:', err);
			patError = $t('settings.general.integrations.github.invalidToken');
			posthog.captureOnboarding(OnboardingEvent.GitHubStorePatFailed);
		}
	}

	function startGitHubEnterpriseFlow() {
		showingFlow = 'ghe';
	}

	async function storeGitHubEnterpriseToken() {
		if (!ghePatInput || !gheHostInput) return;
		ghePatError = undefined;
		gheHostError = undefined;
		try {
			await storeGhePat({ accessToken: ghePatInput, host: gheHostInput });
			posthog.captureOnboarding(OnboardingEvent.GitHubStoreGHEPat);
			cleanupGheFlow();
		} catch (err: any) {
			console.error('Failed to store GitHub Enterprise PAT:', err);
			ghePatError = $t('settings.general.integrations.github.invalidTokenOrHost');
			posthog.captureOnboarding(OnboardingEvent.GitHubStoreGHEPatFailed);
		}
	}
</script>

<div class="stack-v gap-16">
	<CardGroup>
		<ReduxResult result={accounts.result}>
			<!-- IF ERROR -->
			{#snippet error()}
				<CardGroup.Item>
					{#snippet title()}
						{$t('settings.general.integrations.github.loadFailed')}
					{/snippet}
					<Button
						style="pop"
						onclick={deleteAllGitHubAccounts}
						loading={clearingAllResult.current.isLoading}
						>{$t('settings.general.integrations.github.tryAgain')}</Button
					>
					>
				</CardGroup.Item>
			{/snippet}

			<!-- ADD ACCOUNT(S) LIST -->
			{#snippet children(accounts)}
				{@const noAccounts = accounts.length === 0}
				{#each accounts as account}
					<GithubUserLoginState {account} />
				{/each}

				<CardGroup.Item background={accounts.length > 0 ? 'var(--clr-bg-2)' : undefined}>
					{#snippet iconSide()}
						<div class="icon-wrapper__logo">
							{@html githubLogoSvg}
						</div>
					{/snippet}

					{#snippet title()}
						GitHub
					{/snippet}

					{#snippet caption()}
						{$t('settings.general.integrations.github.caption')}
					{/snippet}

					{#snippet actions()}
						{@render addProfileButton(noAccounts)}
					{/snippet}
				</CardGroup.Item>
			{/snippet}
		</ReduxResult>
	</CardGroup>

	<!-- AUTH FLOW -->
	{#if showingFlow === 'oauthFlow'}
		<div in:fade={{ duration: 100 }}>
			<CardGroup.Item standalone>
				<div class="wrapper">
					<div class="step-section">
						<div class="step-line"></div>
						<div class="step-section__content">
							<p class="text-13 text-body">{$t('settings.general.integrations.github.copyCode')}</p>

							<div class="code-wrapper">
								<span class="text-head-20">
									{userCode}
								</span>
								<Button
									style="gray"
									kind="outline"
									icon="copy"
									disabled={codeCopied}
									onclick={() => {
										clipboardService.write(userCode, { message: 'User code copied' });
										codeCopied = true;
									}}
								>
									{$t('settings.general.integrations.github.copyToClipboard')}
								</Button>
							</div>
						</div>
					</div>

					{#if codeCopied}
						<div class="step-section" in:fade={{ duration: 100 }}>
							<div class="step-line step-line-default"></div>
							<div class="step-section__content">
								<p class="text-13 text-body">
									{$t('settings.general.integrations.github.navigateToGitHub')}
								</p>
								<Button
									style="pop"
									disabled={GhActivationLinkPressed}
									icon="open-link"
									onclick={() => {
										urlService.openExternalUrl('https://github.com/login/device');
										GhActivationLinkPressed = true;

										// add timeout to prevent show the check button before the page is opened
										setTimeout(() => {
											GhActivationPageOpened = true;
										}, 500);
									}}
								>
									{$t('settings.general.integrations.github.openGitHub')}
								</Button>
							</div>
						</div>
					{/if}

					{#if GhActivationPageOpened}
						<div class="step-section" in:fade={{ duration: 100 }}>
							<div class="step-line step-line-last"></div>
							<div class="step-section__content">
								<Button
									style="pop"
									{loading}
									disabled={loading}
									onclick={async () => {
										await gitHubOauthCheckStatus(deviceCode);
									}}
								>
									{$t('settings.general.integrations.github.checkStatus')}
								</Button>
							</div>
						</div>
					{/if}
				</div>
			</CardGroup.Item>
		</div>

		<!-- PAT FLOW -->
	{:else if showingFlow === 'pat'}
		<CardGroup>
			<CardGroup.Item>
				{#snippet title()}
					{$t('settings.general.integrations.github.addPat')}
				{/snippet}

				<Textbox
					size="large"
					type="password"
					value={patInput}
					placeholder="ghp_************************"
					oninput={(value) => (patInput = value)}
					error={patError}
				/>
			</CardGroup.Item>
			<CardGroup.Item>
				<div class="flex justify-end gap-6">
					<Button style="gray" kind="outline" onclick={cleanupPatFlow}
						>{$t('settings.general.integrations.github.cancel')}</Button
					>
					<Button
						style="pop"
						disabled={!patInput}
						loading={storePatResult.current.isLoading}
						onclick={storePersonalAccessToken}
					>
						{$t('settings.general.integrations.github.addAccount')}
					</Button>
				</div>
			</CardGroup.Item>
		</CardGroup>
	{:else if showingFlow === 'ghe'}
		<CardGroup>
			<CardGroup.Item>
				{#snippet title()}
					{$t('settings.general.integrations.github.addGhe')}
				{/snippet}

				{#snippet caption()}
					{$t('settings.general.integrations.github.gheCaption')}
				{/snippet}

				<Textbox
					label={$t('settings.general.integrations.github.apiBaseUrl')}
					size="large"
					value={gheHostInput}
					oninput={(value) => (gheHostInput = value)}
					helperText={$t('settings.general.integrations.github.apiBaseUrlHelper')}
					error={gheHostError}
				/>
				<Textbox
					label={$t('settings.general.integrations.github.personalAccessToken')}
					placeholder="ghp_************************"
					size="large"
					type="password"
					value={ghePatInput}
					oninput={(value) => (ghePatInput = value)}
					error={ghePatError}
				/>
			</CardGroup.Item>
			<CardGroup.Item>
				<div class="flex justify-end gap-6">
					<Button style="gray" kind="outline" onclick={cleanupGheFlow}
						>{$t('settings.general.integrations.github.cancel')}</Button
					>
					<Button
						style="pop"
						disabled={!gheHostInput || !ghePatInput}
						loading={storeGhePatResult.current.isLoading}
						onclick={storeGitHubEnterpriseToken}
					>
						{$t('settings.general.integrations.github.addAccount')}
					</Button>
				</div>
			</CardGroup.Item>
		</CardGroup>
	{/if}
</div>

<p class="text-12 text-body github-integration-settings__text">
	{$t('settings.general.integrations.github.credentialsPersisted')}
</p>

{#snippet addProfileButton(noAccounts: boolean)}
	{@const buttonStyle = noAccounts ? 'pop' : 'gray'}
	{@const buttonText = noAccounts
		? $t('settings.general.integrations.github.addAccount')
		: $t('settings.general.integrations.github.addAnotherAccount')}
	<Button
		bind:el={addProfileButtonRef}
		style={buttonStyle}
		onclick={() => addAccountContextMenu?.toggle()}
		disabled={showingFlow !== undefined}
		loading={storePatResult.current.isLoading || storeGhePatResult.current.isLoading}
		icon="plus-small"
	>
		{buttonText}
	</Button>

	<ContextMenu bind:this={addAccountContextMenu} leftClickTrigger={addProfileButtonRef}>
		<ContextMenuSection>
			<ContextMenuItem
				label={$t('settings.general.integrations.github.authorizeAccount')}
				icon="connect-github"
				onclick={() => {
					gitHubStartOauth();
					addAccountContextMenu?.close();
				}}
			/>
			<ContextMenuItem
				label={$t('settings.general.integrations.github.addPat')}
				icon="token-lock"
				onclick={() => {
					startPatFlow();
					addAccountContextMenu?.close();
				}}
			/>
			<ContextMenuItem
				label={$t('settings.general.integrations.github.addGhe')}
				icon="enterprise"
				onclick={() => {
					startGitHubEnterpriseFlow();
					addAccountContextMenu?.close();
				}}
			/>
		</ContextMenuSection>
	</ContextMenu>
{/snippet}

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		width: 100%;
	}

	.icon-wrapper__logo {
		width: 28px;
		height: 28px;
	}

	.step-section {
		display: flex;
		margin-left: 8px;
		gap: 16px;

		&:first-child {
			& .step-section__content {
				&::before {
					display: none;
				}
			}
		}
	}

	.step-section__content {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		width: 100%;
		margin-bottom: 12px;
		margin-left: 8px;
		gap: 12px;

		&:before {
			display: block;
			width: 100%;
			height: 1px;
			margin-top: 8px;
			margin-bottom: 6px;
			background-color: var(--clr-border-1);
			content: '';
			opacity: 0.4;
		}
	}

	/* STEP LINES */
	.step-line {
		position: relative;
		width: 1px;
		margin-top: 4px;
		border-right: 1px dashed var(--clr-border-1);

		&::before {
			position: absolute;
			left: 50%;
			width: 10px;
			height: 10px;
			transform: translateX(-50%);
			border-radius: 100%;
			background-color: var(--clr-border-1);
			content: '';
		}
	}

	.step-line-default {
		&::before {
			top: 28px;
		}
	}

	.step-line-last {
		height: 34px;

		&::before {
			top: 32px;
		}
	}

	.code-wrapper {
		display: flex;
		align-items: center;
		align-self: flex-start;
		padding: 6px 6px 6px 8px;
		gap: 10px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		user-select: text;
	}

	.github-integration-settings__text {
		color: var(--clr-text-2);
	}
</style>

<script lang="ts">
	import GithubUserLoginState from '$components/GithubUserLoginState.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { OnboardingEvent, POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import githubLogoSvg from '$lib/assets/unsized-logos/github.svg?raw';
	import { CLIPBOARD_SERVICE } from '$lib/backend/clipboard';
	import { GITHUB_USER_SERVICE } from '$lib/forge/github/githubUserService.svelte';
	import { URL_SERVICE } from '$lib/utils/url';
	import { inject } from '@gitbutler/core/context';

	import { Button, SectionCard, chipToasts as toasts } from '@gitbutler/ui';
	import { fade } from 'svelte/transition';

	interface Props {
		minimal?: boolean;
	}

	const { minimal = false }: Props = $props();

	const githubUserService = inject(GITHUB_USER_SERVICE);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const posthog = inject(POSTHOG_WRAPPER);

	const [clearAll, clearingAllResult] = githubUserService.deleteAllGitHubAccounts();
	const accounts = githubUserService.accounts();

	// step flags
	let codeCopied = $state(false);
	let GhActivationLinkPressed = $state(false);
	let GhActivationPageOpened = $state(false);
	let showAuthFlow = $state(false);

	let loading = $state(false);
	let userCode = $state('');
	let deviceCode = $state('');

	function gitHubStartOauth() {
		posthog.captureOnboarding(OnboardingEvent.GitHubInitiateOAuth);
		githubUserService.initDeviceOauth().then((verification) => {
			userCode = verification.user_code;
			deviceCode = verification.device_code;
			showAuthFlow = true;
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
			toasts.success('GitHub authenticated');
		} catch (err: any) {
			console.error(err);
			toasts.error('GitHub authentication failed');
			posthog.captureOnboarding(OnboardingEvent.GitHubOAuthFailed);
		} finally {
			// Reset the auth flow on completion
			showAuthFlow = false;
			codeCopied = false;
			GhActivationLinkPressed = false;
			GhActivationPageOpened = false;
			loading = false;
		}
	}

	async function deleteAllGitHubAccounts() {
		await clearAll();
		gitHubStartOauth();
	}
</script>

{#if minimal}
	<Button style="pop" onclick={gitHubStartOauth}>Authorize</Button>
{:else}
	<div class="stack-v gap-16">
		<div class="stack-v">
			<ReduxResult result={accounts.result}>
				{#snippet error()}
					<SectionCard orientation="row">
						{#snippet title()}
							Failed to load GitHub accounts
						{/snippet}
						<Button
							style="pop"
							onclick={deleteAllGitHubAccounts}
							loading={clearingAllResult.current.isLoading}>Try again</Button
						>
					</SectionCard>
				{/snippet}
				{#snippet children(accounts)}
					{#each accounts as account, index}
						<GithubUserLoginState {account} isFirst={index === 0} />
					{/each}

					<SectionCard
						orientation="row"
						background={accounts.length > 0 ? 'disabled' : undefined}
						roundedTop={accounts.length === 0}
					>
						{#snippet iconSide()}
							<div class="icon-wrapper__logo">
								{@html githubLogoSvg}
							</div>
						{/snippet}

						{#snippet title()}
							GitHub
						{/snippet}

						{#snippet caption()}
							Allows you to create Pull Requests
						{/snippet}

						{@const buttonStyle = accounts.length === 0 ? 'pop' : 'neutral'}
						{@const buttonText = accounts.length === 0 ? 'Add account' : 'Add another account'}
						<Button
							style={buttonStyle}
							onclick={gitHubStartOauth}
							disabled={showAuthFlow}
							icon="plus-small">{buttonText}</Button
						>
					</SectionCard>
				{/snippet}
			</ReduxResult>
		</div>

		{#if showAuthFlow}
			<div in:fade={{ duration: 100 }}>
				<SectionCard orientation="row">
					<div class="wrapper">
						<div class="step-section">
							<div class="step-line"></div>
							<div class="step-section__content">
								<p class="text-13 text-body">Copy the following verification code:</p>

								<div class="code-wrapper">
									<span class="text-head-20">
										{userCode}
									</span>
									<Button
										style="neutral"
										kind="outline"
										icon="copy"
										disabled={codeCopied}
										onclick={() => {
											clipboardService.write(userCode, { message: 'User code copied' });
											codeCopied = true;
										}}
									>
										Copy to Clipboard
									</Button>
								</div>
							</div>
						</div>

						{#if codeCopied}
							<div class="step-section" in:fade={{ duration: 100 }}>
								<div class="step-line step-line-default"></div>
								<div class="step-section__content">
									<p class="text-13 text-body">
										Navigate to the GitHub activation page and paste the code you copied.
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
										Open GitHub activation page
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
										Check the status
									</Button>
								</div>
							</div>
						{/if}
					</div>
				</SectionCard>
			</div>
		{/if}
	</div>
{/if}

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
			background-color: var(--clr-scale-ntrl-60);
			content: '';
			opacity: 0.4;
		}
	}

	/* STEP LINES */
	.step-line {
		position: relative;
		width: 1px;
		margin-top: 4px;
		border-right: 1px dashed var(--clr-scale-ntrl-60);

		&::before {
			position: absolute;
			left: 50%;
			width: 10px;
			height: 10px;
			transform: translateX(-50%);
			border-radius: 100%;
			background-color: var(--clr-scale-ntrl-60);
			content: '';
		}
	}

	.step-line-default {
		border-right: 1px dashed var(--clr-scale-ntrl-60);

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
</style>

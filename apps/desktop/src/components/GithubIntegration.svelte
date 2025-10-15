<script lang="ts">
	import GithubUserLoginState from '$components/GithubUserLoginState.svelte';
	import { OnboardingEvent, POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import githubLogoSvg from '$lib/assets/unsized-logos/github.svg?raw';
	import { CLIPBOARD_SERVICE } from '$lib/backend/clipboard';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { GITHUB_USER_SERVICE } from '$lib/forge/github/githubUserService.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { URL_SERVICE } from '$lib/utils/url';
	import { inject } from '@gitbutler/core/context';

	import { Button, Icon, SectionCard, chipToasts as toasts } from '@gitbutler/ui';
	import { fade } from 'svelte/transition';

	interface Props {
		minimal?: boolean;
		disabled?: boolean;
	}

	const { minimal = false, disabled = false }: Props = $props();

	const githubUserService = inject(GITHUB_USER_SERVICE);
	const userService = inject(USER_SERVICE);
	const user = userService.user;
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);
	const posthog = inject(POSTHOG_WRAPPER);
	const appSettings = inject(SETTINGS_SERVICE);
	const usernames = appSettings.knownGitHubUsernames;

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
		if (!$user) return;
		try {
			const { accessToken, login } = await githubUserService.checkAuthStatus({ deviceCode });
			// We don't want to directly modify $user because who knows what state that puts you in
			let mutableUser = structuredClone($user);
			mutableUser.github_access_token = accessToken;
			await userService.setUser(mutableUser);

			// After we call setUser, we want to re-clone the user store, as the userService itself sets the user store
			mutableUser = structuredClone($user);
			mutableUser.github_username = login ?? undefined;
			userService.setUser(mutableUser);
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

	async function forgetGitHub() {
		await githubUserService.forgetGitHubUsernames($usernames);
	}
</script>

{#if minimal}
	<Button style="pop" {disabled} onclick={gitHubStartOauth}>Authorize</Button>
{:else}
	<div class="stack-v gap-8">
		<SectionCard orientation="row">
			{#snippet iconSide()}
				<div class="icon-wrapper">
					{#if $usernames.length > 0}
						<div class="icon-wrapper__tick">
							<Icon name="success" color="success" size={18} />
						</div>
					{/if}
					<div class="icon-wrapper__logo">
						{@html githubLogoSvg}
					</div>
				</div>
			{/snippet}

			{#snippet title()}
				GitHub
			{/snippet}

			{#snippet caption()}
				Allows you to view and create Pull Requests.
			{/snippet}

			{#if $usernames.length > 1}
				<Button kind="outline" {disabled} icon="bin-small" onclick={forgetGitHub} style="error"
					>Forget all</Button
				>
			{:else if $usernames.length === 0}
				<Button style="pop" {disabled} onclick={gitHubStartOauth}>Authorize</Button>
			{/if}
		</SectionCard>
		{#each $usernames as username}
			<GithubUserLoginState {username} {disabled} />
		{/each}

		{#if $usernames.length > 0}
			<div class="centered-row">
				<Button style="pop" disabled={disabled || showAuthFlow} onclick={gitHubStartOauth}
					>Add another account</Button
				>
			</div>
		{/if}

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
		margin-bottom: 8px;
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

	.icon-wrapper {
		position: relative;
		align-self: flex-start;
		height: fit-content;
	}

	.icon-wrapper__tick {
		display: flex;
		position: absolute;
		right: -4px;
		bottom: -4px;
		align-items: center;
		justify-content: center;
		border-radius: 50px;
		background-color: var(--clr-scale-ntrl-100);
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

	.centered-row {
		display: flex;
		justify-content: center;
	}
</style>

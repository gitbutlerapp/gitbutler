<script lang="ts">
	import { CLIPBOARD_SERVICE } from '$lib/backend/clipboard';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { GITHUB_USER_SERVICE } from '$lib/forge/github/githubUserService.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { URL_SERVICE } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';

	import { Button, Icon, Modal, SectionCard, chipToasts as toasts } from '@gitbutler/ui';
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

	// step flags
	let codeCopied = $state(false);
	let GhActivationLinkPressed = $state(false);
	let GhActivationPageOpened = $state(false);

	let loading = $state(false);
	let userCode = $state('');
	let deviceCode = $state('');
	let gitHubOauthModal: ReturnType<typeof Modal> | undefined = $state();

	function gitHubStartOauth() {
		githubUserService.initDeviceOauth().then((verification) => {
			userCode = verification.user_code;
			deviceCode = verification.device_code;
			gitHubOauthModal?.show();
		});
	}

	async function gitHubOauthCheckStatus(deviceCode: string) {
		loading = true;
		if (!$user) return;
		try {
			const accessToken = await githubUserService.checkAuthStatus({ deviceCode });
			// We don't want to directly modify $user because who knows what state that puts you in
			let mutableUser = structuredClone($user);
			mutableUser.github_access_token = accessToken;
			await userService.setUser(mutableUser);

			// After we call setUser, we want to re-clone the user store, as the userService itself sets the user store
			mutableUser = structuredClone($user);
			// TODO: Remove setting of gh username since it isn't used anywhere.
			const githbLogin = await githubUserService.fetchGitHubLogin();
			mutableUser.github_username = githbLogin.name || undefined;
			userService.setUser(mutableUser);
			toasts.success('GitHub authenticated');
		} catch (err: any) {
			console.error(err);
			toasts.error('GitHub authentication failed');
		} finally {
			gitHubOauthModal?.close();
			loading = false;
		}
	}

	function forgetGitHub(): void {
		if ($user) {
			$user.github_access_token = '';
			$user.github_username = '';
			userService.setUser($user);
		}
	}
</script>

{#if minimal}
	<Button style="pop" {disabled} onclick={gitHubStartOauth}>Authorize</Button>
{:else}
	<SectionCard orientation="row" {focusable}>
		{#snippet iconSide()}
			<div class="icon-wrapper">
				{#if $user?.github_access_token}
					<div class="icon-wrapper__tick">
						<Icon name="success" color="success" size={18} />
					</div>
				{/if}
				<svg
					width="28"
					height="28"
					viewBox="0 0 28 28"
					fill="var(--clr-scale-ntrl-0)"
					xmlns="http://www.w3.org/2000/svg"
				>
					<path
						fill-rule="evenodd"
						clip-rule="evenodd"
						d="M14.0116 0C6.26354 0 0 6.41664 0 14.3549C0 20.7004 4.01327 26.0717 9.58073 27.9728C10.2768 28.1157 10.5318 27.6639 10.5318 27.2838C10.5318 26.9511 10.5088 25.8104 10.5088 24.6218C6.61115 25.4776 5.79949 22.9106 5.79949 22.9106C5.17311 21.247 4.245 20.8194 4.245 20.8194C2.96929 19.94 4.33793 19.94 4.33793 19.94C5.75303 20.0351 6.49557 21.4135 6.49557 21.4135C7.74804 23.5998 9.76629 22.9821 10.5782 22.6017C10.6941 21.6748 11.0655 21.0332 11.4599 20.6767C8.3512 20.344 5.08047 19.1082 5.08047 13.5942C5.08047 12.0257 5.63687 10.7423 6.51851 9.74425C6.37941 9.38784 5.89213 7.91405 6.6579 5.94152C6.6579 5.94152 7.84097 5.56119 10.5085 7.41501C11.6506 7.10079 12.8284 6.94094 14.0116 6.9396C15.1947 6.9396 16.4007 7.10614 17.5143 7.41501C20.1822 5.56119 21.3653 5.94152 21.3653 5.94152C22.131 7.91405 21.6435 9.38784 21.5044 9.74425C22.4092 10.7423 22.9427 12.0257 22.9427 13.5942C22.9427 19.1082 19.672 20.32 16.5401 20.6767C17.0506 21.1282 17.4911 21.9837 17.4911 23.3385C17.4911 25.2635 17.4682 26.8084 17.4682 27.2836C17.4682 27.6639 17.7234 28.1157 18.4192 27.9731C23.9867 26.0714 27.9999 20.7004 27.9999 14.3549C28.0229 6.41664 21.7364 0 14.0116 0Z"
					/>
				</svg>
			</div>
		{/snippet}
		{#snippet title()}
			GitHub
		{/snippet}
		{#snippet caption()}
			Allows you to view and create Pull Requests.
		{/snippet}
		{#if $user?.github_access_token}
			<Button kind="outline" {disabled} icon="bin-small" onclick={forgetGitHub}>Forget</Button>
		{:else}
			<Button style="pop" {disabled} onclick={gitHubStartOauth}>Authorize</Button>
		{/if}
	</SectionCard>
{/if}

<Modal
	bind:this={gitHubOauthModal}
	width="small"
	title="Authorize with GitHub"
	onClose={() => {
		codeCopied = false;
		GhActivationLinkPressed = false;
		GhActivationPageOpened = false;
	}}
>
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
			<div class="step-section" transition:fade>
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
			<div class="step-section" transition:fade>
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
</Modal>

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		margin-bottom: 8px;
	}

	.step-section {
		display: flex;
		margin-left: 8px;
		gap: 16px;

		&:last-child {
			& .step-section__content {
				margin-bottom: 0;
				border-bottom: none;
			}
		}

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
		height: 30px;

		&::before {
			top: 30px;
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
</style>

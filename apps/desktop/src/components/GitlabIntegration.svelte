<script lang="ts">
	import GitlabUserLoginState from "$components/GitlabUserLoginState.svelte";
	import ReduxResult from "$components/ReduxResult.svelte";
	import { OnboardingEvent, POSTHOG_WRAPPER } from "$lib/analytics/posthog";
	import gitlabLogoSvg from "$lib/assets/unsized-logos/gitlab.svg?raw";
	import { GITLAB_USER_SERVICE } from "$lib/forge/gitlab/gitlabUserService.svelte";
	import { inject } from "@gitbutler/core/context";

	import { AddForgeAccountButton, Button, CardGroup, Link, Textbox } from "@gitbutler/ui";
	import { fade } from "svelte/transition";

	const gitlabUserService = inject(GITLAB_USER_SERVICE);
	const posthog = inject(POSTHOG_WRAPPER);

	const [clearAll, clearingAllResult] = gitlabUserService.deleteAllGitLabAccounts();
	const [storePat, storePatResult] = gitlabUserService.storeGitLabPat;
	const [storeSelfHostedPat, storeSelfHostedPatResult] = gitlabUserService.storeGitLabEnterprisePat;
	const accounts = gitlabUserService.accounts();

	let showingFlow = $state<"pat" | "selfHosted">();

	// PAT flow state
	let patInput = $state<string>();
	let patError = $state<string>();

	// Self-hosted GitLab flow state
	let selfHostedPatInput = $state<string>();
	let selfHostedHostInput = $state<string>();
	let selfHostedPatError = $state<string>();
	let selfHostedHostError = $state<string>();

	function cleanupPatFlow() {
		showingFlow = undefined;
		patInput = undefined;
		patError = undefined;
	}

	function cleanupSelfHostedFlow() {
		showingFlow = undefined;
		selfHostedPatInput = undefined;
		selfHostedHostInput = undefined;
		selfHostedPatError = undefined;
		selfHostedHostError = undefined;
	}

	async function deleteAllGitLabAccounts() {
		await clearAll();
		startPatFlow();
	}

	function startPatFlow() {
		showingFlow = "pat";
	}

	async function storePersonalAccessToken() {
		if (!patInput) return;
		patError = undefined;
		try {
			await storePat({ accessToken: patInput });
			posthog.captureOnboarding(OnboardingEvent.GitLabStorePat);
			cleanupPatFlow();
		} catch (err: any) {
			console.error("Failed to store GitLab PAT:", err);
			patError = "Invalid token or network error";
			posthog.captureOnboarding(OnboardingEvent.GitLabStorePatFailed);
		}
	}

	function startSelfHostedFlow() {
		showingFlow = "selfHosted";
	}

	async function storeSelfHostedToken() {
		if (!selfHostedPatInput || !selfHostedHostInput) return;
		selfHostedPatError = undefined;
		selfHostedHostError = undefined;
		try {
			await storeSelfHostedPat({ accessToken: selfHostedPatInput, host: selfHostedHostInput });
			posthog.captureOnboarding(OnboardingEvent.GitLabStoreSelfHostedPat);
			cleanupSelfHostedFlow();
		} catch (err: any) {
			console.error("Failed to store self-hosted GitLab PAT:", err);
			selfHostedPatError = "Invalid token or host";
			posthog.captureOnboarding(OnboardingEvent.GitLabStoreSelfHostedPatFailed);
		}
	}
</script>

<div class="stack-v gap-8">
	<CardGroup>
		<ReduxResult result={accounts.result}>
			<!-- IF ERROR -->
			{#snippet error()}
				<CardGroup.Item>
					{#snippet title()}
						Failed to load GitLab accounts
					{/snippet}
					<Button
						style="pop"
						onclick={deleteAllGitLabAccounts}
						loading={clearingAllResult.current.isLoading}>Try again</Button
					>
				</CardGroup.Item>
			{/snippet}

			<!-- ADD ACCOUNT(S) LIST -->
			{#snippet children(accounts)}
				{@const noAccounts = accounts.length === 0}
				{#each accounts as account}
					<GitlabUserLoginState {account} />
				{/each}

				<CardGroup.Item background={accounts.length > 0 ? "var(--clr-bg-2)" : undefined}>
					{#snippet iconSide()}
						<div class="icon-wrapper__logo">
							{@html gitlabLogoSvg}
						</div>
					{/snippet}

					{#snippet title()}
						GitLab
					{/snippet}

					{#snippet caption()}
						Allows you to create Merge Requests
					{/snippet}

					{#snippet actions()}
						{@render addProfileButton(noAccounts)}
					{/snippet}
				</CardGroup.Item>
			{/snippet}
		</ReduxResult>
	</CardGroup>

	<!-- PAT FLOW -->
	{#if showingFlow === "pat"}
		<div in:fade={{ duration: 100 }}>
			<CardGroup>
				<CardGroup.Item>
					{#snippet title()}
						Add Personal Access Token
					{/snippet}

					<Textbox
						size="large"
						type="password"
						value={patInput}
						placeholder="glpat-************************"
						oninput={(value) => (patInput = value)}
						error={patError}
					/>
				</CardGroup.Item>
				<CardGroup.Item>
					<div class="flex justify-end gap-6">
						<Button style="gray" kind="outline" onclick={cleanupPatFlow}>Cancel</Button>
						<Button
							style="pop"
							disabled={!patInput}
							loading={storePatResult.current.isLoading}
							onclick={storePersonalAccessToken}
						>
							Add account
						</Button>
					</div>
				</CardGroup.Item>
			</CardGroup>
		</div>
	{:else if showingFlow === "selfHosted"}
		<div in:fade={{ duration: 100 }}>
			<CardGroup>
				<CardGroup.Item>
					{#snippet title()}
						Add Self-Hosted GitLab Account
					{/snippet}

					{#snippet caption()}
						To connect to your self-hosted GitLab API, allow-list it in the app's CSP settings.
						<br />
						See <Link href="https://docs.gitbutler.com/troubleshooting/custom-csp"
							>docs for details</Link
						>
					{/snippet}

					<Textbox
						label="API Base URL"
						size="large"
						value={selfHostedHostInput}
						oninput={(value) => (selfHostedHostInput = value)}
						helperText="This should be the root URL of the API. For example, if your GitLab instance's hostname is gitlab.acme-inc.com, then set the base URL to https://gitlab.acme-inc.com"
						error={selfHostedHostError}
					/>
					<Textbox
						label="Personal Access Token"
						placeholder="glpat-************************"
						size="large"
						type="password"
						value={selfHostedPatInput}
						oninput={(value) => (selfHostedPatInput = value)}
						error={selfHostedPatError}
					/>
				</CardGroup.Item>
				<CardGroup.Item>
					<div class="flex justify-end gap-6">
						<Button style="gray" kind="outline" onclick={cleanupSelfHostedFlow}>Cancel</Button>
						<Button
							style="pop"
							disabled={!selfHostedHostInput || !selfHostedPatInput}
							loading={storeSelfHostedPatResult.current.isLoading}
							onclick={storeSelfHostedToken}
						>
							Add account
						</Button>
					</div>
				</CardGroup.Item>
			</CardGroup>
		</div>
	{/if}
</div>

<p class="text-12 text-body gitlab-integration-settings__text">
	🔒 Credentials are persisted locally in your OS Keychain / Credential Manager.
</p>

{#snippet addProfileButton(noAccounts: boolean)}
	<AddForgeAccountButton
		{noAccounts}
		disabled={showingFlow !== undefined}
		loading={storePatResult.current.isLoading || storeSelfHostedPatResult.current.isLoading}
		menuItems={[
			{ label: "Add Personal Access Token", icon: "token-lock", onclick: startPatFlow },
			{
				label: "Add Self-Hosted GitLab Account",
				icon: "enterprise",
				onclick: startSelfHostedFlow,
			},
		]}
	/>
{/snippet}

<style lang="postcss">
	.icon-wrapper__logo {
		width: 28px;
		height: 28px;
	}

	.gitlab-integration-settings__text {
		color: var(--clr-text-2);
	}
</style>

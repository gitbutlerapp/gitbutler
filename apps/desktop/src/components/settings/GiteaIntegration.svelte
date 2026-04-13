<script lang="ts">
	import GiteaUserLoginState from "$components/settings/GiteaUserLoginState.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import giteaLogoSvg from "$lib/assets/unsized-logos/gitea.svg?raw";
	import { GITEA_USER_SERVICE } from "$lib/forge/gitea/giteaUserService.svelte";
	import { OnboardingEvent, POSTHOG_WRAPPER } from "$lib/telemetry/posthog";
	import { inject } from "@gitbutler/core/context";
	import { AddForgeAccountButton, Button, CardGroup, Link, Textbox } from "@gitbutler/ui";
	import { fade } from "svelte/transition";

	const giteaUserService = inject(GITEA_USER_SERVICE);
	const posthog = inject(POSTHOG_WRAPPER);

	const [clearAll, clearingAllResult] = giteaUserService.deleteAllGiteaAccounts();
	const [storePat, storePatResult] = giteaUserService.storeGiteaPat;
	const accounts = giteaUserService.accounts();

	let showingFlow = $state<"pat">();

	// PAT flow state
	let patInput = $state<string>();
	let hostInput = $state<string>();
	let patError = $state<string>();
	let hostError = $state<string>();

	function cleanupPatFlow() {
		showingFlow = undefined;
		patInput = undefined;
		hostInput = undefined;
		patError = undefined;
		hostError = undefined;
	}

	async function deleteAllGiteaAccounts() {
		await clearAll();
		startPatFlow();
	}

	function startPatFlow() {
		showingFlow = "pat";
	}

	async function storePersonalAccessToken() {
		if (!patInput || !hostInput) return;
		patError = undefined;
		hostError = undefined;
		try {
			await storePat({ accessToken: patInput, host: hostInput });
			// posthog.captureOnboarding(OnboardingEvent.GiteaStorePat); // Might need to add this event
			cleanupPatFlow();
		} catch (err: any) {
			console.error("Failed to store Gitea PAT:", err);
			patError = "Invalid token or network error";
			// posthog.captureOnboarding(OnboardingEvent.GiteaStorePatFailed);
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
						Failed to load Gitea accounts
					{/snippet}
					<Button
						style="pop"
						onclick={deleteAllGiteaAccounts}
						loading={clearingAllResult.current.isLoading}>Try again</Button
					>
				</CardGroup.Item>
			{/snippet}

			<!-- ADD ACCOUNT(S) LIST -->
			{#snippet children(accounts)}
				{@const noAccounts = accounts.length === 0}
				{#each accounts as account}
					<GiteaUserLoginState {account} />
				{/each}

				<CardGroup.Item background={accounts.length > 0 ? "var(--bg-2)" : undefined}>
					{#snippet iconSide()}
						<div class="icon-wrapper__logo">
							{@html giteaLogoSvg}
						</div>
					{/snippet}

					{#snippet title()}
						Gitea
					{/snippet}

					{#snippet caption()}
						Allows you to create Pull Requests on Gitea
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
						Add Gitea Account
					{/snippet}

					{#snippet caption()}
						To connect to your Gitea instance, provide the Host URL and a Personal Access Token.
						<br />
						See <Link href="https://docs.gitbutler.com/features/forge-integration/gitea-integration"
							>docs for details</Link
						>
					{/snippet}

					<Textbox
						label="Gitea Host URL"
						size="large"
						value={hostInput}
						placeholder="https://gitea.example.com"
						oninput={(value) => (hostInput = value)}
						error={hostError}
					/>

					<Textbox
						label="Personal Access Token"
						size="large"
						type="password"
						value={patInput}
						placeholder="Token created in Settings -> Applications"
						oninput={(value) => (patInput = value)}
						error={patError}
					/>
				</CardGroup.Item>
				<CardGroup.Item>
					<div class="flex justify-end gap-6">
						<Button style="gray" kind="outline" onclick={cleanupPatFlow}>Cancel</Button>
						<Button
							style="pop"
							disabled={!patInput || !hostInput}
							loading={storePatResult.current.isLoading}
							onclick={storePersonalAccessToken}
						>
							Add account
						</Button>
					</div>
				</CardGroup.Item>
			</CardGroup>
		</div>
	{/if}
</div>

<p class="text-12 text-body gitea-integration-settings__text">
	🔒 Credentials are persisted locally in your OS Keychain / Credential Manager.
</p>

{#snippet addProfileButton(noAccounts: boolean)}
	<AddForgeAccountButton
		{noAccounts}
		disabled={showingFlow !== undefined}
		loading={storePatResult.current.isLoading}
		menuItems={[
			{ label: "Add Gitea Account", icon: "lock-auth", onclick: startPatFlow },
		]}
	/>
{/snippet}

<style lang="postcss">
	.icon-wrapper__logo {
		width: 28px;
		height: 28px;
	}

	.gitea-integration-settings__text {
		color: var(--text-2);
	}
</style>

<script lang="ts">
	import GiteaUserLoginState from "$components/GiteaUserLoginState.svelte";
	import ReduxResult from "$components/ReduxResult.svelte";
	import { GITEA_USER_SERVICE } from "$lib/forge/gitea/giteaUserService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, CardGroup, Textbox } from "@gitbutler/ui";
	import { fade } from "svelte/transition";

	const giteaUserService = inject(GITEA_USER_SERVICE);

	const [clearAll, clearingAllResult] = giteaUserService.deleteAllGiteaAccounts();
	const [storeAccount, storeAccountResult] = giteaUserService.storeGiteaAccount;
	const accounts = giteaUserService.accounts();

	let showingFlow = $state(false);
	let hostInput = $state<string>();
	let patInput = $state<string>();
	let hostError = $state<string>();
	let patError = $state<string>();

	function cleanupFlow() {
		showingFlow = false;
		hostInput = undefined;
		patInput = undefined;
		hostError = undefined;
		patError = undefined;
	}

	function startFlow() {
		showingFlow = true;
	}

	async function deleteAllGiteaAccounts() {
		await clearAll();
		startFlow();
	}

	async function storeToken() {
		if (!hostInput || !patInput) return;
		hostError = undefined;
		patError = undefined;
		try {
			await storeAccount({ host: hostInput, accessToken: patInput });
			cleanupFlow();
		} catch (err: any) {
			console.error("Failed to store Gitea token:", err);
			hostError = "Invalid host or network error";
			patError = "Invalid token";
		}
	}
</script>

<div class="stack-v gap-8">
	<CardGroup>
		<ReduxResult result={accounts.result}>
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

			{#snippet children(accounts)}
				{@const noAccounts = accounts.length === 0}
				{#each accounts as account}
					<GiteaUserLoginState {account} />
				{/each}

				<CardGroup.Item background={accounts.length > 0 ? "var(--clr-bg-2)" : undefined}>
					{#snippet iconSide()}
						<div class="icon-wrapper__logo">GT</div>
					{/snippet}

					{#snippet title()}
						Gitea
					{/snippet}

					{#snippet caption()}
						Store a personal access token for Codeberg or any Gitea-compatible instance.
					{/snippet}

					{#snippet actions()}
						<Button
							style="pop"
							onclick={startFlow}
							disabled={showingFlow}
							loading={storeAccountResult.current.isLoading}
						>
							{noAccounts ? "Add account" : "Add another"}
						</Button>
					{/snippet}
				</CardGroup.Item>
			{/snippet}
		</ReduxResult>
	</CardGroup>

	{#if showingFlow}
		<div in:fade={{ duration: 100 }}>
			<CardGroup>
				<CardGroup.Item>
					{#snippet title()}
						Add Gitea Account
					{/snippet}

					{#snippet caption()}
						Use the web base URL for your instance, such as `https://codeberg.org` or your
						self-hosted Gitea host.
					{/snippet}

					<Textbox
						label="Instance URL"
						size="large"
						value={hostInput}
						oninput={(value) => (hostInput = value)}
						placeholder="https://codeberg.org"
						error={hostError}
					/>
					<Textbox
						label="Personal Access Token"
						size="large"
						type="password"
						value={patInput}
						oninput={(value) => (patInput = value)}
						error={patError}
					/>
				</CardGroup.Item>
				<CardGroup.Item>
					<div class="flex justify-end gap-6">
						<Button style="gray" kind="outline" onclick={cleanupFlow}>Cancel</Button>
						<Button
							style="pop"
							disabled={!hostInput || !patInput}
							loading={storeAccountResult.current.isLoading}
							onclick={storeToken}
						>
							Add account
						</Button>
					</div>
				</CardGroup.Item>
			</CardGroup>
		</div>
	{/if}
</div>

<p class="text-12 text-body integration-settings__text">
	🔒 Credentials are persisted locally in your OS Keychain / Credential Manager.
</p>

<style lang="postcss">
	.icon-wrapper__logo {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border-radius: 999px;
		background: var(--clr-bg-3);
		color: var(--clr-text-1);
		font-weight: 700;
		font-size: 11px;
		letter-spacing: 0.08em;
	}

	.integration-settings__text {
		color: var(--clr-text-2);
	}
</style>

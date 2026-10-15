<script lang="ts">
	// forgejo-fork: mirrors BitbucketIntegration, with an instance URL instead of an email.
	import ForgejoUserLoginState from "$components/settings/ForgejoUserLoginState.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import forgejoLogoSvg from "$lib/assets/unsized-logos/forgejo.svg?raw";
	import { isNormalizedError } from "$lib/error/normalizedError";
	import { FORGEJO_USER_SERVICE } from "$lib/forge/forgejo/forgejoUserService.svelte";
	import { inject } from "@gitbutler/core/context";

	import { AddForgeAccountButton, Button, CardGroup, Link, Textbox } from "@gitbutler/ui";
	import { fade } from "svelte/transition";

	const forgejoUserService = inject(FORGEJO_USER_SERVICE);

	const [clearAll, clearingAllResult] = forgejoUserService.deleteAllForgejoAccounts();
	const [storePat, storePatResult] = forgejoUserService.storeForgejoPat;
	const accounts = forgejoUserService.accounts();

	let showingFlow = $state<"pat">();

	let hostInput = $state<string>();
	let tokenInput = $state<string>();
	let tokenError = $state<string>();

	// Where to create a token on the entered instance, once it looks like a host.
	const tokenSettingsUrl = $derived.by(() => {
		const host = hostInput?.trim().replace(/\/+$/, "");
		if (!host || !host.includes(".")) return undefined;
		const origin = host.includes("://") ? host : `https://${host}`;
		return `${origin}/user/settings/applications`;
	});

	function cleanupPatFlow() {
		showingFlow = undefined;
		hostInput = undefined;
		tokenInput = undefined;
		tokenError = undefined;
	}

	async function deleteAllForgejoAccounts() {
		await clearAll();
		showingFlow = "pat";
	}

	async function storeForgejoPat() {
		if (!hostInput || !tokenInput) return;
		tokenError = undefined;
		try {
			await storePat({ host: hostInput, accessToken: tokenInput });
			cleanupPatFlow();
		} catch (err: unknown) {
			console.error("Failed to store Forgejo token:", err);
			// The backend explains rejected tokens and wrong URLs in the message.
			tokenError = isNormalizedError(err) ? err.message : "Invalid token or instance URL";
		}
	}
</script>

<div class="stack-v gap-8">
	<CardGroup>
		<ReduxResult result={accounts.result}>
			{#snippet error()}
				<CardGroup.Item>
					{#snippet title()}
						Failed to load Forgejo accounts
					{/snippet}
					<Button
						style="pop"
						onclick={deleteAllForgejoAccounts}
						loading={clearingAllResult.current.isLoading}>Try again</Button
					>
				</CardGroup.Item>
			{/snippet}

			{#snippet children(accounts)}
				{@const noAccounts = accounts.length === 0}
				{#each accounts as account}
					<ForgejoUserLoginState {account} />
				{/each}

				<CardGroup.Item background={accounts.length > 0 ? "var(--bg-2)" : undefined}>
					{#snippet iconSide()}
						<div class="icon-wrapper__logo">
							{@html forgejoLogoSvg}
						</div>
					{/snippet}

					{#snippet title()}
						Forgejo
					{/snippet}

					{#snippet caption()}
						Allows you to create Pull Requests on Codeberg or any self-hosted Forgejo instance
					{/snippet}

					{#snippet actions()}
						<AddForgeAccountButton
							{noAccounts}
							disabled={showingFlow !== undefined}
							loading={storePatResult.current.isLoading}
							menuItems={[
								{
									label: "Add Personal Access Token",
									icon: "lock-auth",
									onclick: () => (showingFlow = "pat"),
								},
							]}
						/>
					{/snippet}
				</CardGroup.Item>
			{/snippet}
		</ReduxResult>
	</CardGroup>

	{#if showingFlow === "pat"}
		<div in:fade={{ duration: 100 }}>
			<CardGroup>
				<CardGroup.Item>
					{#snippet title()}
						Add Forgejo Personal Access Token
					{/snippet}

					{#snippet caption()}
						Requires the read:user, write:repository and write:issue scopes.
						{#if tokenSettingsUrl}
							<br />
							<Link href={tokenSettingsUrl}>Create one in your Forgejo settings</Link>
						{/if}
					{/snippet}

					<Textbox
						label="Instance URL"
						size="large"
						value={hostInput}
						placeholder="https://codeberg.org"
						oninput={(value) => (hostInput = value)}
					/>
					<Textbox
						label="Personal access token"
						size="large"
						type="password"
						value={tokenInput}
						placeholder="••••••••••••••••••••••••••••••••••••••••"
						oninput={(value) => (tokenInput = value)}
						error={tokenError}
					/>
				</CardGroup.Item>
				<CardGroup.Item>
					<div class="flex justify-end gap-6">
						<Button style="gray" kind="outline" onclick={cleanupPatFlow}>Cancel</Button>
						<Button
							style="pop"
							disabled={!hostInput || !tokenInput}
							loading={storePatResult.current.isLoading}
							onclick={storeForgejoPat}
						>
							Add account
						</Button>
					</div>
				</CardGroup.Item>
			</CardGroup>
		</div>
	{/if}
</div>

<p class="text-12 text-body forgejo-integration-settings__text">
	🔒 Credentials are persisted locally in your OS Keychain / Credential Manager.
</p>

<style lang="postcss">
	.icon-wrapper__logo {
		width: 28px;
		height: 28px;
	}

	.forgejo-integration-settings__text {
		color: var(--text-2);
	}
</style>

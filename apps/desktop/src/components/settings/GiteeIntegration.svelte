<script lang="ts">
	import GiteeUserLoginState from "$components/settings/GiteeUserLoginState.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import giteeLogoSvg from "$lib/assets/unsized-logos/gitee.svg?raw";
	import { giteePatError, GITEE_USER_SERVICE } from "$lib/forge/gitee/giteeUserService.svelte";
	import { t } from "$lib/i18n";
	import { inject } from "@gitbutler/core/context";

	import { AddForgeAccountButton, Button, CardGroup, Link, Textbox } from "@gitbutler/ui-svelte";
	import { fade } from "svelte/transition";

	const giteeUserService = inject(GITEE_USER_SERVICE);

	const [storePat, storePatResult] = giteeUserService.storeGiteePat;
	const accounts = giteeUserService.accounts();

	let showingFlow = $state<"pat">();

	// PAT flow state
	let patInput = $state<string>();
	let patError = $state<string>();

	function cleanupPatFlow() {
		showingFlow = undefined;
		patInput = undefined;
		patError = undefined;
	}

	function startPatFlow() {
		showingFlow = "pat";
	}

	async function storePersonalAccessToken() {
		if (!patInput) return;
		patError = undefined;
		try {
			await storePat({ accessToken: patInput });
			cleanupPatFlow();
		} catch (err: unknown) {
			console.error("Failed to store Gitee PAT:", err);
			patError = giteePatError(err);
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
						{t("failed-to-load-gitee-accounts")}
					{/snippet}
					<Button style="pop" onclick={() => location.reload()}>{t("try-again")}</Button>
				</CardGroup.Item>
			{/snippet}

			<!-- ADD ACCOUNT(S) LIST -->
			{#snippet children(accounts)}
				{@const noAccounts = accounts.length === 0}
				{#each accounts as account}
					<GiteeUserLoginState {account} />
				{/each}

				<CardGroup.Item background={accounts.length > 0 ? "var(--bg-2)" : undefined}>
					{#snippet iconSide()}
						<div class="icon-wrapper__logo">
							{@html giteeLogoSvg}
						</div>
					{/snippet}

					{#snippet title()}
						Gitee
					{/snippet}

					{#snippet caption()}
						{t("allows-you-to-create-pull-requests")}
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
						{t("add-personal-access-token")}
					{/snippet}

					{#snippet caption()}
						{t("create-a-token-at")}
						<Link href="https://gitee.com/profile/personal_access_tokens"
							>gitee.com/profile/personal_access_tokens</Link
						>
						with <span class="text-bold">projects</span> scope.
					{/snippet}

					<Textbox
						size="large"
						type="password"
						value={patInput}
						placeholder="personal-access-token"
						oninput={(value) => (patInput = value)}
						error={patError}
					/>
				</CardGroup.Item>
				<CardGroup.Item>
					<div class="flex justify-end gap-6">
						<Button style="gray" kind="outline" onclick={cleanupPatFlow}>{t("cancel")}</Button>
						<Button
							style="pop"
							disabled={!patInput}
							loading={storePatResult.current.isLoading}
							onclick={storePersonalAccessToken}
						>
							{t("add-account")}
						</Button>
					</div>
				</CardGroup.Item>
			</CardGroup>
		</div>
	{/if}
</div>

{#snippet addProfileButton(noAccounts: boolean)}
	<AddForgeAccountButton
		{noAccounts}
		disabled={showingFlow !== undefined}
		loading={storePatResult.current.isLoading}
		menuItems={[{ label: "Add Personal Access Token", icon: "lock-auth", onclick: startPatFlow }]}
	/>
{/snippet}

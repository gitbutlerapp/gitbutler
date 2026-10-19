<script lang="ts">
	import SectionCardDisclaimer from "$components/shared/SectionCardDisclaimer.svelte";
	import { GIT_CONFIG_SERVICE } from "$lib/config/gitConfigService";
	import { classify } from "$lib/error/errorClassification";
	import { parseError } from "$lib/error/parser";
	import { t } from "$lib/i18n";
	import { OnboardingEvent, POSTHOG_WRAPPER } from "$lib/telemetry/posthog";
	import { inject } from "@gitbutler/core/context";
	import { Button, Icon, InfoMessage, Link } from "@gitbutler/ui-svelte";
	import { slide } from "svelte/transition";

	interface Props {
		projectId: string;
		disabled: boolean;
		remoteName: string | null | undefined;
		branchName: string | null | undefined;
	}

	const { projectId, remoteName, branchName, disabled }: Props = $props();

	const gitConfig = inject(GIT_CONFIG_SERVICE);
	const posthog = inject(POSTHOG_WRAPPER);

	type Check = { name: string; promise: Promise<any> };
	let checks = $state<Check[]>();

	let errors = $state(0);

	let loading = $state(false);

	async function checkCredentials() {
		if (!remoteName || !branchName) return;
		posthog.capture(OnboardingEvent.GitCheckCredentials);
		loading = true;
		errors = 0;
		checks = [];
		let stage: "fetch" | "push" = "fetch";

		try {
			const fetchCheck = gitConfig.checkGitFetch(projectId, remoteName);
			checks = [{ name: "Fetch", promise: fetchCheck }];
			await fetchCheck;
			stage = "push";
			const pushCheck = gitConfig.checkGitPush(projectId, remoteName, branchName);
			checks = [...checks, { name: "Push", promise: pushCheck }];
			await pushCheck;
		} catch (error) {
			const { code, userMessage } = classify(error);
			posthog.captureOnboarding(
				OnboardingEvent.GitCheckCredentialsFailed,
				{
					name: "Git credential check failed",
					message: userMessage ?? "Git credential check failed.",
					code,
				},
				{ stage },
			);
			errors = 1;
		} finally {
			loading = false;
		}
	}

	export function reset() {
		checks = [];
	}
</script>

<div class="credential-check">
	{#if checks && checks.length > 0}
		<div transition:slide={{ duration: 250 }}>
			<InfoMessage
				style={errors > 0 ? "warning" : loading ? "info" : "success"}
				filled
				outlined={false}
			>
				{#snippet title()}
					{#if loading}
						{t("checking-git-credentials")}
					{:else if errors > 0}
						{t("there-was-a-problem-with-your-credentials")}
					{:else}
						{t("all-checks-passed-successfully")}
					{/if}
				{/snippet}

				{#snippet content()}
					<div class="checks-list" transition:slide={{ duration: 250, delay: 1000 }}>
						{#if checks}
							{#each checks as check}
								<div class="text-12 text-body check-result">
									<i class="check-icon">
										{#await check.promise}
											<Icon name="spinner" size={14} />
										{:then}
											<Icon name="tick" size={14} />
										{:catch}
											<Icon name="danger" size={14} />
										{/await}
									</i>{check.name}

									{#await check.promise catch err}
										- {parseError(err).message}
									{/await}
								</div>
							{/each}
						{/if}
					</div>

					{#if errors > 0}
						<div class="text-12 text-body help-text" transition:slide>
							<span>
								{t("try-another-setting-and-test-again")}
								<br />
								{t("consult-our")}
								<Link href="https://docs.gitbutler.com/troubleshooting/fetch-push">
									{t("fetch-push-guide")}
								</Link>
								{t("for-help-fixing-this-problem")}
							</span>
						</div>
					{/if}
				{/snippet}
			</InfoMessage>
		</div>
	{/if}
	<Button style="pop" wide icon="tick" {loading} {disabled} onclick={checkCredentials}>
		{#if loading || checks?.length === 0}
			{t("test-credentials")}
		{:else}
			{t("re-test-credentials")}
		{/if}
	</Button>
	<SectionCardDisclaimer>
		{t("to-test-the-push-command-we-create-an-empty-branch-and-promp")}
		<Link href="https://docs.gitbutler.com/troubleshooting/fetch-push">{t("read-more")}</Link>
		{t("about-authentication-methods")}
	</SectionCardDisclaimer>
</div>

<style>
	.credential-check {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.checks-list {
		display: flex;
		flex-direction: column;
		margin-top: 4px;
		gap: 4px;
	}

	.check-icon {
		display: flex;
	}

	.check-result {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.help-text {
		margin-top: 6px;
	}
</style>

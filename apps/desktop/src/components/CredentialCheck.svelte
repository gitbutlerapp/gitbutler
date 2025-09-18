<script lang="ts">
	import InfoMessage from '$components/InfoMessage.svelte';
	import SectionCardDisclaimer from '$components/SectionCardDisclaimer.svelte';
	import { OnboardingEvent, POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import { GIT_CONFIG_SERVICE } from '$lib/config/gitConfigService';
	import { inject } from '@gitbutler/core/context';
	import { Button, Icon, Link } from '@gitbutler/ui';
	import { slide } from 'svelte/transition';

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

		try {
			const fetchCheck = gitConfig.checkGitFetch(projectId, remoteName);
			checks = [{ name: 'Fetch', promise: fetchCheck }];
			await fetchCheck;
			const pushCheck = gitConfig.checkGitPush(projectId, remoteName, branchName);
			checks = [...checks, { name: 'Push', promise: pushCheck }];
			await pushCheck;
		} catch {
			posthog.capture(OnboardingEvent.GitCheckCredentialsFailed);
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
				style={errors > 0 ? 'warning' : loading ? 'info' : 'success'}
				filled
				outlined={false}
			>
				{#snippet title()}
					{#if loading}
						Checking git credentials …
					{:else if errors > 0}
						There was a problem with your credentials
					{:else}
						All checks passed successfully
					{/if}
				{/snippet}

				{#snippet content()}
					<div class="checks-list" transition:slide={{ duration: 250, delay: 1000 }}>
						{#if checks}
							{#each checks as check}
								<div class="text-12 text-body check-result">
									<i class="check-icon">
										{#await check.promise}
											<Icon name="spinner" spinnerRadius={4} />
										{:then}
											<Icon name="success-small" color="success" />
										{:catch}
											<Icon name="error-small" color="error" />
										{/await}
									</i>{check.name}

									{#await check.promise catch err}
										- {err}
									{/await}
								</div>
							{/each}
						{/if}
					</div>

					{#if errors > 0}
						<div class="text-12 text-body help-text" transition:slide>
							<span>
								Try another setting and test again?
								<br />
								Consult our
								<Link href="https://docs.gitbutler.com/troubleshooting/fetch-push">
									fetch / push guide
								</Link>
								for help fixing this problem.
							</span>
						</div>
					{/if}
				{/snippet}
			</InfoMessage>
		</div>
	{/if}
	<Button style="pop" wide icon="item-tick" {loading} {disabled} onclick={checkCredentials}>
		{#if loading || checks?.length === 0}
			Test credentials
		{:else}
			Re-test credentials
		{/if}
	</Button>
	<SectionCardDisclaimer>
		To test the push command, we create an empty branch and promptly remove it after the check. <Link
			href="https://docs.gitbutler.com/troubleshooting/fetch-push">Read more</Link
		> about authentication methods.
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

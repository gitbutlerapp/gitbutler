<script lang="ts">
	import SectionCardDisclaimer from '../components/SectionCardDisclaimer.svelte';
	import Icon from '../shared/Icon.svelte';
	import InfoMessage from '../shared/InfoMessage.svelte';
	import Link from '../shared/Link.svelte';
	import { AuthService } from '$lib/backend/auth';
	import Button from '$lib/shared/Button.svelte';
	import { getContext } from '$lib/utils/context';
	import { slide } from 'svelte/transition';

	export let projectId: string;
	export let remoteName: string | null | undefined;
	export let branchName: string | null | undefined;

	const authService = getContext(AuthService);

	type Check = { name: string; promise: Promise<any> };
	$: checks = [] as Check[];

	$: errors = 0;
	$: loading = false;

	async function checkCredentials() {
		if (!remoteName || !branchName) return;
		loading = true;
		errors = 0;
		checks = [];

		try {
			const fetchCheck = authService.checkGitFetch(projectId, remoteName);
			checks = [{ name: 'Fetch', promise: fetchCheck }];
			await fetchCheck;
			const pushCheck = authService.checkGitPush(projectId, remoteName, branchName);
			checks = [...checks, { name: 'Push', promise: pushCheck }];
			await pushCheck;
		} catch {
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
	{#if checks.length > 0}
		<div transition:slide={{ duration: 250 }}>
			<InfoMessage
				style={errors > 0 ? 'warning' : loading ? 'neutral' : 'success'}
				filled
				outlined={false}
			>
				<svelte:fragment slot="title">
					{#if loading}
						Checking git credentials …
					{:else if errors > 0}
						There was a problem with your credentials
					{:else}
						All checks passed successfully
					{/if}
				</svelte:fragment>

				<svelte:fragment slot="content">
					<div class="checks-list" transition:slide={{ duration: 250, delay: 1000 }}>
						{#each checks as check}
							<div class="text-base-body-12 check-result">
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
					</div>

					{#if errors > 0}
						<div class="text-base-body-12 help-text" transition:slide>
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
				</svelte:fragment>
			</InfoMessage>
		</div>
	{/if}
	<Button
		style="pop"
		kind="solid"
		wide
		icon="item-tick"
		disabled={loading}
		on:click={checkCredentials}
	>
		{#if loading || checks.length === 0}
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
		gap: 4px;
		margin-top: 4px;
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

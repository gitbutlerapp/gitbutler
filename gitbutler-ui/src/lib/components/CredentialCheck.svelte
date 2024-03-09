<script lang="ts">
	import Button from './Button.svelte';
	import Icon from './Icon.svelte';
	import InfoMessage, { type MessageStyle } from './InfoMessage.svelte';
	import Link from './Link.svelte';
	import { slide } from 'svelte/transition';
	import type { AuthService } from '$lib/backend/auth';

	export let authService: AuthService;
	export let projectId: string;
	export let remoteName: string | null | undefined;
	export let branchName: string | null | undefined;

	$: success = true;
	$: loading = false;
	$: errors = 0;
	$: passes = 0;
	$: style = checkToStyle(loading, success, errors);

	type Check = { name: string; promise: Promise<any> };
	$: checks = [] as Check[];

	function checkToStyle(loading: boolean, success: boolean, errors: number): MessageStyle {
		if (success) return 'success';
		if (errors > 0) return 'warn';
		if (loading) return 'neutral';
		return 'neutral';
	}

	const isRejected = (input: PromiseSettledResult<unknown>): input is PromiseRejectedResult =>
		input.status === 'rejected';

	const isFulfilled = <T,>(
		input: PromiseSettledResult<unknown>
	): input is PromiseFulfilledResult<T> => input.status === 'fulfilled';

	async function checkCredentials() {
		success = false;
		passes = 0;
		errors = 0;

		if (!remoteName || !branchName) {
			return;
		}

		loading = true;
		try {
			checks = [
				{
					name: 'Fetch',
					promise: authService.checkGitFetch(projectId, remoteName).catch((reason) => {
						++errors;
						throw reason;
					})
				},
				{ name: 'Push', promise: authService.checkGitPush(projectId, remoteName, branchName) }
			];
			const results = await Promise.allSettled(checks.map((c) => c.promise));
			errors = results.filter(isRejected).map((r) => `${r.reason}`).length;
			passes = results.filter(isFulfilled).map((r) => `${r.value}`).length;
			setTimeout(() => (success = errors == 0), 1250);
		} finally {
			loading = false;
		}
	}
</script>

<div class="credential-check">
	{#if checks.length > 0}
		<div transition:slide={{ duration: 250 }}>
			<InfoMessage {style} filled outlined={false}>
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
					{#if loading || (checks.length > 0 && (errors > 0 || (errors == 0 && passes == 0)))}
						<div class="checks-list" transition:slide={{ duration: 250, delay: 1000 }}>
							{#each checks as check}
								<div class="check-result">
									{#await check.promise}
										<Icon name="spinner" spinnerRadius={3.5} />
									{:then}
										<Icon name="success-small" color="success" />
									{:catch}
										<Icon name="error-small" color="error" />
									{/await}
									{check.name}
									{#await check.promise catch err}
										- {err}
									{/await}
								</div>
							{/each}
						</div>
					{/if}
					{#if errors > 0}
						<div class="help-text" transition:slide>
							Try another setting and test again? You can also refer to our
							<Link href="https://docs.gitbutler.com/troubleshooting/fetch-push">
								fetch & pull documentation
							</Link>
							for help fixing this problem.
						</div>
					{/if}
				</svelte:fragment>
			</InfoMessage>
		</div>
	{/if}
	<Button wide icon="test" disabled={loading} on:click={checkCredentials}>
		{#if loading || checks.length == 0}
			Test credentials
		{:else}
			Re-test credentials
		{/if}
	</Button>
	<div class="disclaimer">
		To test the push command, we create an empty branch and promptly remove it after the check. <Link
			href="https://docs.gitbutler.com/troubleshooting/fetch-push">Read more</Link
		> about authentication methods.
	</div>
</div>

<style>
	.credential-check {
		display: flex;
		flex-direction: column;
		gap: var(--space-16);
	}

	.checks-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
		padding-left: var(--space-4);
		margin-top: var(--space-12);
	}
	.check-result {
		display: flex;
		gap: var(--space-6);
	}
	.help-text {
		margin-top: var(--space-6);
	}

	.disclaimer {
		color: var(--clr-theme-scale-ntrl-50);
		background: var(--clr-theme-container-pale);
		border-radius: var(--m, 6px);
		background: var(--clr-theme-container-pale);
		padding: var(--space-10) var(--space-12);
	}
</style>

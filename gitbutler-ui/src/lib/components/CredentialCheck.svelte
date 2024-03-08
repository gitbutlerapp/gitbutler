<script lang="ts">
	import Button from './Button.svelte';
	import InfoMessage, { type MessageStyle } from './InfoMessage.svelte';
	import Link from './Link.svelte';
	import { slide } from 'svelte/transition';
	import type { AuthService, GitCredentialCheck } from '$lib/backend/auth';

	export let authService: AuthService;
	export let projectId: string;
	export let remoteName: string | null | undefined;
	export let branchName: string | null | undefined;

	let credentialsCheck: any | undefined;
	let loading = false;

	let fetchError: string | undefined;
	let pushError: string | undefined;
	let success = false;

	$: style = checkToStyle(credentialsCheck);

	function checkToStyle(check: GitCredentialCheck | undefined): MessageStyle {
		if (check?.ok) return 'success';
		if (check?.error) return 'warn';
		return 'neutral';
	}

	async function checkCredentials() {
		loading = true;
		success = false;
		await checkPush();
		await checkFetch();
		loading = false;
		success = true;
	}

	async function checkFetch() {
		credentialsCheck = undefined;
		loading = true;
		credentialsCheck = await authService.checkGitFetch(projectId, remoteName);
		if (credentialsCheck.error) {
			fetchError = credentialsCheck.error;
		}
		loading = false;
	}

	async function checkPush() {
		credentialsCheck = undefined;
		loading = true;
		credentialsCheck = await authService.checkGitPush(projectId, remoteName, branchName);
		if (credentialsCheck.error) {
			pushError = credentialsCheck.error;
		}
		loading = false;
	}
</script>

<div class="credential-check">
	{#if success || fetchError || pushError}
		<div transition:slide>
			<InfoMessage {style} filled outlined={false}>
				<svelte:fragment slot="title">
					{#if loading}
						Checking git credentials …
					{:else if fetchError}
						Unable to fetch
					{:else if pushError}
						Unable to push
					{:else}
						All checks passed successfully
					{/if}
				</svelte:fragment>
				<svelte:fragment>
					{#if fetchError}
						We were unable to fetch from the remote, please check your authentication settings.
						<Link href="https://docs.gitbutler.com/troubleshooting/fetch-push">Learn more</Link>.
						{fetchError}
					{:else if pushError}
						We were unable to push to the remote, please check your authentication settings.
						<Link href="https://docs.gitbutler.com/troubleshooting/fetch-push">Learn more</Link>.
						{pushError}
					{/if}
				</svelte:fragment>
			</InfoMessage>
		</div>
	{/if}
	<Button wide icon="test" {loading} disabled={loading} on:click={checkCredentials}>
		Test credentials
	</Button>
	<div class="disclaimer">
		To test the push command, we create an empty branch and promptly remove it after the check.
	</div>
</div>

<style>
	.credential-check {
		display: flex;
		flex-direction: column;
		gap: var(--space-16);
	}

	.disclaimer {
		color: var(--clr-theme-scale-ntrl-50);
		background: var(--clr-theme-container-pale);
		border-radius: var(--m, 6px);
		background: var(--container-pale, #f4f4f4);
		padding: var(--space-10) var(--space-12);
	}
</style>

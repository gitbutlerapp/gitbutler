<script lang="ts">
	import Button from './Button.svelte';
	import InfoMessage, { type MessageStyle } from './InfoMessage.svelte';
	import Link from './Link.svelte';
	import { showToast } from '$lib/notifications/toasts';
	import { slide } from 'svelte/transition';
	import type { AuthService, GitCredentialCheck } from '$lib/backend/auth';

	export let authService: AuthService;
	export let projectId: string;
	export let remoteName: string | undefined;
	export let branchName: string | undefined;

	let credentialsCheck: any | undefined;
	let loading = false;

	$: style = checkToStyle(credentialsCheck);

	function checkToStyle(check: GitCredentialCheck | undefined): MessageStyle {
		if (check?.ok) return 'success';
		if (check?.error) return 'warn';
		return 'neutral';
	}

	async function checkCredentials() {
		return await Promise.all([checkPush(), checkFetch()]);
	}

	async function checkFetch() {
		credentialsCheck = undefined;
		loading = true;
		credentialsCheck = await authService.checkGitFetch(projectId, remoteName);
		if (credentialsCheck.error) {
			showToast({ title: 'Failed to fetch from remote', message: credentialsCheck.error });
		}
		loading = false;
	}

	async function checkPush() {
		credentialsCheck = undefined;
		loading = true;
		credentialsCheck = await authService.checkGitPush(projectId, remoteName, branchName);
		if (credentialsCheck.error) {
			showToast({ title: 'Failed to push to remote', message: credentialsCheck.error });
		}
		loading = false;
	}
</script>

<div class="credential-check">
	{#if credentialsCheck}
		<div transition:slide>
			<InfoMessage {style} filled outlined={false}>
				<svelte:fragment slot="title">
					{#if loading}
						Checking git credentials …
					{:else if credentialsCheck.ok}
						All checks have passed successfully
					{:else if credentialsCheck.error}
						Unable to Fetch and Push
					{/if}
				</svelte:fragment>
				<svelte:fragment>
					{#if credentialsCheck.error}
						No worries! You can easily fix this by adjusting your Git authentication in the project
						settings.
						<Link href="https://docs.gitbutler.com/troubleshooting/fetch-push">Learn more</Link>.
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

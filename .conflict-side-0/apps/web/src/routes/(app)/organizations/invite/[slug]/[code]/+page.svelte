<script lang="ts">
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import { browser } from '$app/environment';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { env } from '$env/dynamic/public';

	const userService = getContext(UserService);
	const organizationService = getContext(OrganizationService);

	const user = $derived(userService.user);

	// Get the org slug and invite code from the route parameters
	const inviteCode = $derived($page.params.code);
	const slug = $derived($page.params.slug);

	// Get services from context
	const routes = getContext(WebRoutesService);

	// Track the auth and join status
	let isLoggedIn = $state(false);
	let isJoining = $state(false);
	let joinError = $state<string | null>(null);
	let joinSuccess = $state(false);
	let showConfirmation = $state(false);

	// Check auth status and respond accordingly
	$effect(() => {
		isLoggedIn = !!$user?.id;
	});

	// Process the invite when authenticated
	async function processInvite() {
		if (!isLoggedIn) return;

		isJoining = true;
		joinError = null;

		try {
			// Use the organizationService instead of direct fetch
			await organizationService.joinOrganization(slug, inviteCode);

			joinSuccess = true;

			// Redirect to the organization page after successful join
			setTimeout(() => {
				goto(routes.ownerPath({ ownerSlug: slug }));
			}, 1500);
		} catch (error: any) {
			// Try to extract error message from JSON response if available
			let errorMessage = 'Failed to join organization';
			try {
				// Check if error has a response with JSON data
				if (error.response && error.response.data) {
					// Extract error message from JSON response
					errorMessage = error.response.data.error || errorMessage;
				} else if (typeof error.message === 'string') {
					// Try to parse error message as JSON if it's a string
					const errorJson = JSON.parse(error.message.replace(/^[^{]*/, ''));
					errorMessage = errorJson.error || errorMessage;
				}
			} catch (_) {
				// If JSON parsing fails, use the original error message
				errorMessage = error.message || errorMessage;
			}

			joinError = errorMessage;
			isJoining = false;
		}
	}

	$effect(() => {
		if (browser && isLoggedIn) {
			showConfirmation = true;
		}
	});

	// Handle confirmation to join
	function handleConfirm() {
		processInvite();
	}

	// Handle manual retry
	function handleRetry() {
		processInvite();
	}

	// Navigate to login if not logged in
	function goToLogin() {
		// Store the current URL in session storage to redirect back after login
		if (browser) {
			sessionStorage.setItem('redirectAfterLogin', window.location.href);
		}
		window.location.href = `${env.PUBLIC_APP_HOST}/cloud/login?callback=${window.location.href}`;
	}
</script>

<div class="invite-container">
	<div class="invite-card">
		<h1>Organization Invitation</h1>

		{#if !isLoggedIn}
			<p>You've been invited to join <strong>{slug}</strong>.</p>
			<p>Please log in to continue.</p>
			<Button onclick={goToLogin} style="pop">Log In</Button>
		{:else if isJoining}
			<div class="loading-container">
				<p>Joining organization...</p>
			</div>
		{:else if joinError}
			<p>{joinError}</p>
			<div class="button-container">
				<Button onclick={handleRetry} style="pop">Try Again</Button>
			</div>
		{:else if joinSuccess}
			<p>You have successfully joined the organization.</p>
			<p>Redirecting to the organization page...</p>
		{:else if showConfirmation}
			<p>You've been invited to join <strong>{slug}</strong>.</p>
			<p>Would you like to accept this invitation?</p>
			<div class="button-container">
				<Button onclick={handleConfirm} style="pop">Join Organization</Button>
			</div>
		{:else}
			<p>Processing your invitation...</p>
		{/if}
	</div>
</div>

<style>
	.invite-container {
		display: flex;
		justify-content: center;
		align-items: center;
		min-height: 70vh;
		padding: 2rem;
	}

	.invite-card {
		background-color: var(--color-bg-card);
		border-radius: 0.5rem;
		padding: 2rem;
		box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
		max-width: 500px;
		width: 100%;
		text-align: center;
	}

	h1 {
		margin-bottom: 1.5rem;
		font-size: 1.5rem;
	}

	.loading-container {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1rem;
	}

	.button-container {
		margin-top: 1.5rem;
	}
</style>

<script lang="ts">
	import { browser } from '$app/environment';
	import { inject } from '@gitbutler/shared/context';
	import {
		OrganizationService,
		ORGANIZATION_SERVICE
	} from '@gitbutler/shared/organizations/organizationService';
	import Button from '@gitbutler/ui/Button.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';

	interface Props {
		organizationSlug: string;
		inviteCode: string;
	}

	let { organizationSlug, inviteCode }: Props = $props();

	let inviteUrl = $state('');
	let copied = $state(false);
	let resetting = $state(false);
	let serviceError = $state(false);

	// Get the OrganizationService from context
	const organizationService = inject(ORGANIZATION_SERVICE);

	$effect(() => {
		if (browser) {
			updateInviteUrl();
		}
	});

	function updateInviteUrl() {
		// Create the invite URL with the origin of the current page
		const baseUrl = window.location.origin;
		inviteUrl = `${baseUrl}/organizations/invite/${organizationSlug}/${inviteCode}`;
	}

	function copyToClipboard() {
		if (!browser) return;

		// Use the Clipboard API to copy the invite URL
		navigator.clipboard
			.writeText(inviteUrl)
			.then(() => {
				copied = true;
				setTimeout(() => {
					copied = false;
				}, 2000);
			})
			.catch((err) => {
				console.error('Failed to copy:', err);
			});
	}

	async function resetInviteCode() {
		if (!browser) return;

		// Show confirmation dialog
		const confirmed = confirm(
			'Are you sure you want to reset the invite code? This will invalidate all existing invite links.'
		);

		if (confirmed) {
			try {
				resetting = true;
				const updatedOrg = await (organizationService as OrganizationService).resetInviteCode(
					organizationSlug
				);

				// Update the invite code from the result
				inviteCode = updatedOrg.inviteCode || '';

				// Update the invite URL with the new code
				updateInviteUrl();
			} catch (error) {
				console.error('Failed to reset invite code:', error);
				alert('Failed to reset invite code. Please try again.');
			} finally {
				resetting = false;
			}
		}
	}
</script>

{#if inviteCode}
	<div class="invite-link-container">
		<p>Share this link to invite people to join this organization:</p>

		<div class="invite-url-container">
			<Textbox readonly value={inviteCode} />
			<Button onclick={copyToClipboard} style={copied ? 'success' : 'pop'}>copy url</Button>
		</div>

		<p class="info-text">
			Anyone with this link can join your organization by accepting the invitation.
		</p>

		<div class="reset-container">
			<Button onclick={resetInviteCode} style="warning" disabled={resetting || serviceError}>
				{#if serviceError}
					Service Unavailable
				{:else if resetting}
					Resetting...
				{:else}
					Reset Invite Code
				{/if}
			</Button>
			{#if serviceError}
				<p class="error-text">
					Reset functionality is currently unavailable. The organization service could not be found.
				</p>
			{/if}
		</div>
	</div>
{/if}

<style>
	.invite-link-container {
		padding: 1rem;
	}

	p {
		margin-bottom: 1rem;
		font-size: 0.9rem;
	}

	.invite-url-container {
		display: flex;
		margin-bottom: 0.75rem;
	}

	.info-text {
		margin-top: 0.5rem;
		color: #718096;
		font-style: italic;
		font-size: 0.8rem;
	}

	.reset-container {
		margin-top: 1.5rem;
		padding-top: 1rem;
		border-top: 1px solid #e2e8f0;
	}

	.error-text {
		margin-top: 0.5rem;
		color: #e53e3e;
		font-weight: bold;
		font-size: 0.8rem;
	}
</style>

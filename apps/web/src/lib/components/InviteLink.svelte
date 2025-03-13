<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import { onMount } from 'svelte';
	import { browser } from '$app/environment';

	interface Props {
		organizationSlug: string;
		inviteCode: string;
	}

	let { organizationSlug, inviteCode }: Props = $props();

	let inviteUrl = $state('');
	let copied = $state(false);

	onMount(() => {
		if (browser) {
			// Create the invite URL with the origin of the current page
			const baseUrl = window.location.origin;
			inviteUrl = `${baseUrl}/organizations/invite/${organizationSlug}/${inviteCode}`;
		}
	});

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
</script>

<div class="invite-link-container">
	<p>Share this link to invite people to join this organization:</p>

	<div class="invite-url-container">
		<Textbox readonly value={inviteUrl} />
		<Button onclick={copyToClipboard} style={copied ? 'success' : 'pop'}>copy</Button>
	</div>

	<p class="info-text">
		Anyone with this link can join your organization by accepting the invitation.
	</p>
</div>

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
		color: #718096;
		font-style: italic;
		font-size: 0.8rem;
		margin-top: 0.5rem;
	}
</style>

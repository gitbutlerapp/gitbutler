<script lang="ts">
	import { getContext } from '$lib/context';
	import { OrganizationService } from '$lib/organizations/organizationService';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';

	const organizationService = getContext(OrganizationService);

	let modal = $state<Modal>();

	let organizationSlug = $state('');
	let joinCode = $state('');
	let joiningState = $state<'intert' | 'loading' | 'completed'>('intert');
	const buttonEnabled = $derived(!!(joinCode && organizationSlug));

	async function join(close: () => void) {
		joiningState = 'loading';

		await organizationService.joinOrganization(organizationSlug, joinCode);

		joiningState = 'completed';
		close();
	}

	export function show() {
		modal?.show();
	}
</script>

<Modal bind:this={modal} title="Join an organization" width="small">
	{#snippet children()}
		<p>To join an organization, you need to have a join code and an organization slug.</p>
		<br />
		<Textbox bind:value={organizationSlug} label="Organization slug" />
		<br />
		<Textbox bind:value={joinCode} label="Join code" />
	{/snippet}

	{#snippet controls(close)}
		<Button
			disabled={!buttonEnabled}
			loading={joiningState === 'loading'}
			onclick={() => join(close)}>Join</Button
		>
	{/snippet}
</Modal>

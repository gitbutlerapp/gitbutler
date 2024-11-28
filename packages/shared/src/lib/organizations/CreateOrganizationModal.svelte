<script lang="ts">
	import { getContext } from '$lib/context';
	import { OrganizationService } from '$lib/organizations/organizationService';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import { slugify } from '@gitbutler/ui/utils/string';

	const organizationService = getContext(OrganizationService);

	let name = $state<string>('');
	let slug = $state<string>('');
	const sluggifiedSlug = $derived(slugify(slug || name).toLocaleLowerCase());
	let description = $state<string>('');

	const requiredFieldsFilled = $derived(!!(name && sluggifiedSlug));
	let modalCreationState: 'inert' | 'loading' | 'complete' = $state('inert');

	function onModalClose() {
		name = '';
		slug = '';
		description = '';
		modalCreationState = 'inert';
	}

	async function create(close: () => void) {
		modalCreationState = 'loading';
		await organizationService.createOrganization(sluggifiedSlug, name, description);
		modalCreationState = 'complete';
		close();
	}

	let modal = $state<Modal>();

	export function show() {
		modal?.show();
	}
</script>

<Modal bind:this={modal} onClose={onModalClose}>
	<Textbox bind:value={name} label="Name" required></Textbox>
	<Textbox bind:value={slug} label="Slug" required></Textbox>
	{#if slug !== sluggifiedSlug}
		<p>Slug will be save as: {sluggifiedSlug}</p>
	{/if}

	<Textarea bind:value={description} label="Description"></Textarea>

	{#snippet controls(close)}
		<Button
			disabled={!requiredFieldsFilled}
			loading={modalCreationState === 'loading'}
			onclick={() => create(close)}>Create</Button
		>
	{/snippet}
</Modal>

<script lang="ts">
	import { ORGANIZATION_SERVICE } from '$lib/organizations/organizationService';
	import { inject } from '@gitbutler/core/context';

	import { Button, Modal, Textarea, Textbox } from '@gitbutler/ui';
	import { slugify } from '@gitbutler/ui/utils/string';

	const organizationService = inject(ORGANIZATION_SERVICE);

	let name = $state<string>('');
	let slug = $state<string>('');
	const sluggifiedSlug = $derived(slugify(slug || name || '').toLocaleLowerCase());
	let description = $state<string>('');

	const requiredFieldsFilled = $derived(!!(name && sluggifiedSlug));
	let modalCreationState: 'inert' | 'loading' | 'complete' = $state('inert');
	let submitAttempted = $state(false);

	function onModalClose() {
		name = '';
		slug = '';
		description = '';
		modalCreationState = 'inert';
		submitAttempted = false;
	}

	async function create(close: () => void) {
		submitAttempted = true;

		if (!requiredFieldsFilled) return;

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
	<div class="form-container">
		<h2>Create a new Organization</h2>
		<p>
			Organizations are a way to group projects and collaborate with your team. You can create as
			many organizations as you want.
		</p>
		<Textbox bind:value={name} label="Name" required={submitAttempted}></Textbox>
		<Textbox bind:value={slug} label="Slug" required={submitAttempted}></Textbox>
		{#if slug !== sluggifiedSlug}
			<p>Slug will be save as: {sluggifiedSlug}</p>
		{/if}

		<Textarea bind:value={description} label="Description"></Textarea>
	</div>

	{#snippet controls(close)}
		<Button
			disabled={!requiredFieldsFilled}
			loading={modalCreationState === 'loading'}
			onclick={() => create(close)}>Create</Button
		>
	{/snippet}
</Modal>

<style>
	h2 {
		margin-bottom: 10px;
		font-weight: 600;
		font-size: 20px;
	}

	.form-container {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
</style>

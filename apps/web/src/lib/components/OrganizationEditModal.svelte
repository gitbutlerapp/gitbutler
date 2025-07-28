<script lang="ts">
	import { inject } from '@gitbutler/shared/context';
	import { ORGANIZATION_SERVICE } from '@gitbutler/shared/organizations/organizationService';

	import { Button, Modal, Textarea, Textbox } from '@gitbutler/ui';
	import chipToasts from '@gitbutler/ui/toasts';
	import { slugify } from '@gitbutler/ui/utils/string';

	interface Props {
		organizationSlug: string;
		onUpdate?: (newSlug: string) => void;
	}

	let { organizationSlug, onUpdate = () => {} }: Props = $props();

	// Get organization service from context
	const organizationService = inject(ORGANIZATION_SERVICE);

	// Form state
	let name = $state('');
	let slug = $state('');
	let description = $state('');
	let originalSlug = $state('');

	// Derived slugified value
	const sluggifiedSlug = $derived(slugify(slug).toLocaleLowerCase());

	// Form state
	let isLoading = $state(false);
	let submitAttempted = $state(false);
	const requiredFieldsFilled = $derived(!!(name && sluggifiedSlug));

	// The modal component reference
	let modal = $state<Modal>();

	// Function to fetch organization details
	async function fetchOrganizationDetails() {
		try {
			isLoading = true;

			const organization = await organizationService.getOrganizationBySlug(organizationSlug);

			if (organization) {
				name = organization.name || '';
				slug = organization.slug;
				originalSlug = organization.slug;
				description = organization.description || '';
			}
		} catch (error) {
			chipToasts.error(
				`Failed to fetch organization details: ${error instanceof Error ? error.message : 'Unknown error'}`
			);
		} finally {
			isLoading = false;
		}
	}

	// Function to update organization details
	async function updateOrganization(close: () => void) {
		submitAttempted = true;

		if (!requiredFieldsFilled) return;

		try {
			isLoading = true;

			const hasSlugChanged = originalSlug !== sluggifiedSlug;

			await organizationService.updateOrganization(originalSlug, {
				name,
				new_slug: hasSlugChanged ? sluggifiedSlug : undefined, // Only send new_slug if it changed
				description
			});

			chipToasts.success('Organization updated successfully');

			// Notify parent component about the update
			onUpdate(sluggifiedSlug);

			close();

			// If the slug changed, we might need to redirect or refresh
			if (hasSlugChanged) {
				// Handle slug change - might require page refresh or redirect
				window.location.href = window.location.href.replace(originalSlug, sluggifiedSlug);
			}
		} catch (error) {
			chipToasts.error(
				`Failed to update organization: ${error instanceof Error ? error.message : 'Unknown error'}`
			);
		} finally {
			isLoading = false;
		}
	}

	// Reset form on modal close
	function onModalClose() {
		submitAttempted = false;
	}

	// Public function to show the modal
	export function show() {
		fetchOrganizationDetails();
		modal?.show();
	}
</script>

<Modal bind:this={modal} title="Edit Organization" onClose={onModalClose} width="medium">
	<div class="form-container">
		<Textbox bind:value={name} label="Name" required={submitAttempted} disabled={isLoading} />

		<Textbox bind:value={slug} label="Slug" required={submitAttempted} disabled={isLoading} />

		{#if slug !== sluggifiedSlug}
			<p class="slug-note">Slug will be saved as: {sluggifiedSlug}</p>
		{/if}

		<Textarea bind:value={description} label="Description" disabled={isLoading} />
	</div>

	{#snippet controls(close)}
		<Button kind="outline" onclick={close} disabled={isLoading}>Cancel</Button>

		<Button
			style="pop"
			disabled={!requiredFieldsFilled || isLoading}
			loading={isLoading}
			onclick={() => updateOrganization(close)}
		>
			Save Changes
		</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.form-container {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.slug-note {
		margin-top: -12px;
		color: var(--text-muted, #666);
		font-size: 13px;
	}
</style>

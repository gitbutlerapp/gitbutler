<script lang="ts">
	import Section from '$components/Section.svelte';
	import { inject } from '@gitbutler/shared/context';
	import RegisterInterest from '@gitbutler/shared/interest/RegisterInterest.svelte';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { HTTP_CLIENT } from '@gitbutler/shared/network/httpClient';
	import CreateOrganizationModal from '@gitbutler/shared/organizations/CreateOrganizationModal.svelte';
	import JoinOrganizationModal from '@gitbutler/shared/organizations/JoinOrganizationModal.svelte';
	import OrganizationModal from '@gitbutler/shared/organizations/OrganizationModal.svelte';
	import { ORGANIZATION_SERVICE } from '@gitbutler/shared/organizations/organizationService';
	import { organizationTable } from '@gitbutler/shared/organizations/organizationsSlice';
	import { APP_STATE } from '@gitbutler/shared/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';

	const organizationService = inject(ORGANIZATION_SERVICE);
	const appState = inject(APP_STATE);
	const httpClient = inject(HTTP_CLIENT);
	const authenticated = httpClient.authenticationAvailable;

	const organizationsInterest = organizationService.getOrganizationListingInterest();
	const organizations = $derived(organizationTable.selectors.selectAll(appState.organizations));

	let createOrganizationModal = $state<CreateOrganizationModal>();
</script>

{#if $authenticated}
	<RegisterInterest interest={organizationsInterest} />
{/if}

<CreateOrganizationModal bind:this={createOrganizationModal} />

<JoinOrganizationModal />
<Button onclick={() => createOrganizationModal?.show()}>Create an Organizaton</Button>

<Section gap={0}>
	{#each organizations as loadableOrganization, index (loadableOrganization.id)}
		<SectionCard
			roundedTop={index === 0}
			roundedBottom={index === organizations.length - 1}
			orientation="row"
		>
			<Loading loadable={loadableOrganization}>
				{#snippet children(organization)}
					<div class="inline">
						<p class="text-15 text-bold">{organization.name || organization.slug}</p>
						{#if organization.name}
							<p class="text-13">{organization.slug}</p>
						{/if}
					</div>
				{/snippet}
			</Loading>

			{#snippet actions()}
				<OrganizationModal slug={loadableOrganization.id} />
			{/snippet}
		</SectionCard>
	{/each}
</Section>

<style lang="postcss">
	.inline {
		display: flex;

		flex-grow: 1;
		align-items: center;

		gap: 8px;
	}
</style>

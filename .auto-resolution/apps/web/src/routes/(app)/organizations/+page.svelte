<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import RegisterInterest from '@gitbutler/shared/interest/RegisterInterest.svelte';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { HttpClient } from '@gitbutler/shared/network/httpClient';
	import CreateOrganizationModal from '@gitbutler/shared/organizations/CreateOrganizationModal.svelte';
	import JoinOrganizationModal from '@gitbutler/shared/organizations/JoinOrganizationModal.svelte';
	import OrganizationModal from '@gitbutler/shared/organizations/OrganizationModal.svelte';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { organizationsSelectors } from '@gitbutler/shared/organizations/organizationsSlice';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';

	const organizationService = getContext(OrganizationService);
	const appState = getContext(AppState);
	const httpClient = getContext(HttpClient);
	const authenticated = httpClient.authenticationAvailable;

	const organizationsInterest = organizationService.getOrganizationListingInterest();
	const organizations = $derived(organizationsSelectors.selectAll(appState.organizations));

	let createOrganizationModal = $state<CreateOrganizationModal>();
</script>

{#if $authenticated}
	<RegisterInterest interest={organizationsInterest} />
{/if}

<CreateOrganizationModal bind:this={createOrganizationModal} />
<JoinOrganizationModal />

<Button onclick={() => createOrganizationModal?.show()}>Create an Organizaton</Button>

<div>
	{#each organizations as organization, index (organization.id)}
		<Loading loadable={organization}>
			{#snippet children(organization)}
				<SectionCard
					roundedTop={index === 0}
					roundedBottom={index === organizations.length - 1}
					orientation="row"
				>
					{#snippet children()}
						<div class="inline">
							<p class="text-15 text-bold">{organization.name || organization.slug}</p>
							{#if organization.name}
								<p class="text-13">{organization.slug}</p>
							{/if}
						</div>
					{/snippet}

					{#snippet actions()}
						<OrganizationModal slug={organization.slug} />
					{/snippet}
				</SectionCard>
			{/snippet}
		</Loading>
	{/each}
</div>

<style lang="postcss">
	.inline {
		display: flex;
		align-items: center;

		gap: 8px;

		flex-grow: 1;
	}
</style>

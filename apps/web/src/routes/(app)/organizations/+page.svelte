<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import RegisterInterest from '@gitbutler/shared/interest/RegisterInterest.svelte';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { HttpClient } from '@gitbutler/shared/network/httpClient';
	import CreateOrganizationModal from '@gitbutler/shared/organizations/CreateOrganizationModal.svelte';
	import JoinOrganizationModal from '@gitbutler/shared/organizations/JoinOrganizationModal.svelte';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { organizationTable } from '@gitbutler/shared/organizations/organizationsSlice';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';

	const organizationService = getContext(OrganizationService);
	const appState = getContext(AppState);
	const httpClient = getContext(HttpClient);
	const authenticated = httpClient.authenticationAvailable;

	const organizationsInterest = organizationService.getOrganizationListingInterest();
	const organizations = $derived(organizationTable.selectors.selectAll(appState.organizations));

	let joinOrganizationModal: JoinOrganizationModal;
	let createOrganizationModal: CreateOrganizationModal;

	function getOrganizationProjectsCount(organization: any) {
		return organization.projectRepositoryIds?.length || 0;
	}

	function getOrganizationMembersCount(organization: any) {
		return organization.memberLogins?.length || 0;
	}

	function navigateToOrganization(slug: string) {
		window.location.href = `/${slug}`;
	}
</script>

{#if $authenticated}
	<RegisterInterest interest={organizationsInterest} />
{/if}

<JoinOrganizationModal bind:this={joinOrganizationModal} />
<CreateOrganizationModal bind:this={createOrganizationModal} />

<div class="page-container">
	<header class="page-header">
		<div class="page-title">
			<Icon name="settings" />
			<h1>Organizations</h1>
		</div>
		<p class="page-description">Manage your organizations and team collaboration settings</p>
	</header>

	<div class="content-wrapper">
		<div class="main-content">
			{#if organizations.length > 0}
				<div class="organizations-header">
					<h2>Your Organizations</h2>
					<Button style="pop" onclick={() => createOrganizationModal?.show()}
						>Create an Organization</Button
					>
				</div>

				<div class="organizations-list">
					{#each organizations as organization, index (organization.id)}
						<Loading loadable={organization}>
							{#snippet children(organization)}
								<SectionCard
									roundedTop={index === 0}
									roundedBottom={index === organizations.length - 1}
									orientation="row"
								>
									{#snippet children()}
										<div class="organization-card">
											<div
												class="organization-avatar"
												style:background-color="var(--clr-scale-ntrl-20)"
												style:color="var(--clr-scale-ntrl-80)"
											>
												<span>{(organization.name || organization.slug)[0].toUpperCase()}</span>
											</div>
											<div class="organization-info">
												<div class="organization-name-row">
													<h3 class="organization-name">
														{organization.name || organization.slug}
													</h3>
													{#if organization.name}
														<p class="organization-slug">@{organization.slug}</p>
													{/if}
												</div>
												<div class="organization-stats">
													<div class="stat">
														<Icon name="profile" />
														<span>{getOrganizationMembersCount(organization)} members</span>
													</div>
													<div class="stat">
														<Icon name="search" />
														<span>{getOrganizationProjectsCount(organization)} projects</span>
													</div>
												</div>
											</div>
										</div>
									{/snippet}

									{#snippet actions()}
										<div class="organization-actions">
											<Button
												kind="outline"
												onclick={() => navigateToOrganization(organization.slug)}
											>
												View
											</Button>
										</div>
									{/snippet}
								</SectionCard>
							{/snippet}
						</Loading>
					{/each}
				</div>
			{:else}
				<div class="empty-state-wrapper">
					<EmptyStatePlaceholder>
						{#snippet title()}
							No Organizations Yet
						{/snippet}
						{#snippet caption()}
							Create your first organization to collaborate with your team
						{/snippet}
					</EmptyStatePlaceholder>
					<div class="empty-state-action">
						<Button style="pop" onclick={() => createOrganizationModal?.show()}
							>Create an Organization</Button
						>
					</div>
				</div>
			{/if}
		</div>
	</div>

	<!-- Mobile Join Organization section -->
	<div class="join-section">
		<div class="join-card">
			<h3 class="join-title">Join an Organization</h3>
			<p class="join-description">Have an invitation code? Join an existing organization.</p>
			<Button style="pop" onclick={() => joinOrganizationModal?.show()}>Join Organization</Button>
		</div>
	</div>
</div>

<style lang="postcss">
	.page-container {
		max-width: 1200px;
		min-width: 900px;
		margin: 0 auto;
		padding: 24px;
	}

	@media (max-width: 900px) {
		.page-container {
			min-width: 90%;
		}
	}

	.page-header {
		margin-bottom: 32px;
	}

	.page-title {
		display: flex;
		align-items: center;
		gap: 12px;
		margin-bottom: 8px;
	}

	.page-title h1 {
		font-size: 24px;
		font-weight: 600;
		color: var(--clr-text-1);
	}

	.page-description {
		color: var(--clr-text-2);
		font-size: 14px;
	}

	.content-wrapper {
		display: flex;
		gap: 24px;
	}

	.main-content {
		flex: 1;
		min-width: 0; /* Prevent flex item from overflowing */
	}

	.organizations-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 16px;
	}

	.organizations-header h2 {
		font-size: 18px;
		font-weight: 500;
		color: var(--clr-text-1);
	}

	.organizations-list {
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.organization-card {
		display: flex;
		align-items: center;
		gap: 16px;
		flex-grow: 1;
	}

	.organization-avatar {
		width: 40px;
		height: 40px;
		border-radius: 8px;
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 600;
		font-size: 18px;
		color: var(--clr-text-inverse);
	}

	.organization-info {
		display: flex;
		flex-direction: column;
		gap: 4px;
		flex-grow: 1;
	}

	.organization-name-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.organization-name {
		font-size: 16px;
		font-weight: 600;
		color: var(--clr-text-1);
	}

	.organization-slug {
		font-size: 14px;
		color: var(--clr-text-2);
	}

	.organization-stats {
		display: flex;
		gap: 16px;
	}

	.stat {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		color: var(--clr-text-3);
	}

	.organization-actions {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.empty-state-wrapper {
		display: flex;
		flex-direction: column;
		align-items: center;
		margin-top: 64px;
	}

	.empty-state-action {
		margin-top: 24px;
	}

	.join-section {
		margin-top: 32px;
	}

	.join-card {
		background: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		padding: 20px;
		text-align: center;
	}

	.join-title {
		font-size: 16px;
		font-weight: 600;
		color: var(--clr-text-1);
		margin-bottom: 8px;
	}

	.join-description {
		font-size: 14px;
		color: var(--clr-text-2);
		margin-bottom: 16px;
	}
</style>

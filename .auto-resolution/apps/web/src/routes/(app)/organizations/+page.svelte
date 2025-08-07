<script lang="ts">
	import { inject } from '@gitbutler/shared/context';
	import RegisterInterest from '@gitbutler/shared/interest/RegisterInterest.svelte';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { HTTP_CLIENT } from '@gitbutler/shared/network/httpClient';
	import CreateOrganizationModal from '@gitbutler/shared/organizations/CreateOrganizationModal.svelte';
	import JoinOrganizationModal from '@gitbutler/shared/organizations/JoinOrganizationModal.svelte';
	import { ORGANIZATION_SERVICE } from '@gitbutler/shared/organizations/organizationService';
	import { organizationTable } from '@gitbutler/shared/organizations/organizationsSlice';
	import { APP_STATE } from '@gitbutler/shared/redux/store.svelte';
	import { Button, EmptyStatePlaceholder, Icon, SectionCard } from '@gitbutler/ui';

	const organizationService = inject(ORGANIZATION_SERVICE);
	const appState = inject(APP_STATE);
	const httpClient = inject(HTTP_CLIENT);
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
		min-width: 900px;
		max-width: 1200px;
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
		margin-bottom: 8px;
		gap: 12px;
	}

	.page-title h1 {
		color: var(--clr-text-1);
		font-weight: 600;
		font-size: 24px;
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
		align-items: center;
		justify-content: space-between;
		margin-bottom: 16px;
	}

	.organizations-header h2 {
		color: var(--clr-text-1);
		font-weight: 500;
		font-size: 18px;
	}

	.organizations-list {
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.organization-card {
		display: flex;
		flex-grow: 1;
		align-items: center;
		gap: 16px;
	}

	.organization-avatar {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 40px;
		height: 40px;
		border-radius: 8px;
		color: var(--clr-text-inverse);
		font-weight: 600;
		font-size: 18px;
	}

	.organization-info {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		gap: 4px;
	}

	.organization-name-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.organization-name {
		color: var(--clr-text-1);
		font-weight: 600;
		font-size: 16px;
	}

	.organization-slug {
		color: var(--clr-text-2);
		font-size: 14px;
	}

	.organization-stats {
		display: flex;
		gap: 16px;
	}

	.stat {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--clr-text-3);
		font-size: 13px;
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
		padding: 20px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
		text-align: center;
	}

	.join-title {
		margin-bottom: 8px;
		color: var(--clr-text-1);
		font-weight: 600;
		font-size: 16px;
	}

	.join-description {
		margin-bottom: 16px;
		color: var(--clr-text-2);
		font-size: 14px;
	}
</style>

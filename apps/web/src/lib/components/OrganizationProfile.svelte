<script lang="ts">
	import InviteLink from '$lib/components/InviteLink.svelte';
	import ProjectsSection from '$lib/components/ProjectsSection.svelte';
	import ReviewsSection from '$lib/components/ReviewsSection.svelte';
	import { OwnerService } from '$lib/owner/ownerService';
	import { UserService } from '$lib/user/userService';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import { getContext } from 'svelte';
	import type { ExtendedOrganization } from '$lib/owner/types';
	import type { HttpClient } from '@gitbutler/shared/network/httpClient';
	import type { AppDispatch } from '@gitbutler/shared/redux/store.svelte';

	interface Props {
		organization: ExtendedOrganization;
		ownerSlug: string;
	}

	let { organization, ownerSlug }: Props = $props();

	// Get organization service from context
	const organizationService =
		(getContext(OrganizationService) as OrganizationService) ||
		new OrganizationService(
			getContext('HttpClient') as HttpClient,
			getContext('AppDispatch') as AppDispatch
		);

	// Get user service from context
	const userService = getContext(UserService) as UserService;

	// Get owner service to refresh organization data
	const ownerService = getContext(OwnerService) as OwnerService;

	let patchStacksStore = $state(organizationService.getPatchStacks(ownerSlug));
	let patchStacks = $derived($patchStacksStore);
	let patchStacksData = $derived(patchStacks.status === 'found' ? patchStacks.value || [] : []);

	// Create a local mutable copy of the organization for updates
	let localOrganization = $state(organization);

	// Get current user information
	let currentUser = $state(userService.user);
	let currentUserLogin = $derived($currentUser?.login);

	// Modal for confirmation
	let confirmRemoveUserModal = $state<Modal>();
	let userToRemove = $state<string | null>(null);
	let isRemoving = $state(false);

	// Function to refresh organization data
	async function refreshOrganizationData() {
		// Refresh patch stacks
		patchStacksStore = organizationService.getPatchStacks(ownerSlug);

		// Directly fetch the updated organization data to reflect changes immediately
		try {
			const response = await ownerService.fetchOwner(ownerSlug);
			if (response.type === 'organization') {
				// Update our local copy
				localOrganization = response.data;
			}
		} catch (error) {
			console.error('Failed to refresh organization data:', error);
		}
	}

	// Function to handle removal of a user
	async function removeUser() {
		if (!userToRemove || !ownerSlug) return;

		isRemoving = true;
		try {
			await organizationService.removeUser(ownerSlug, userToRemove);

			// Remove the user locally to update UI immediately
			if (localOrganization.members) {
				localOrganization.members = localOrganization.members.filter(
					(member) => member.login !== userToRemove
				);
			}

			// Refresh data from server to ensure consistency
			await refreshOrganizationData();
		} catch (error) {
			console.error('Failed to remove user:', error);
		} finally {
			isRemoving = false;
			userToRemove = null;
			confirmRemoveUserModal?.close();
		}
	}

	// Function to open removal confirmation modal
	function confirmRemoveUserDialog(login: string) {
		userToRemove = login;
		confirmRemoveUserModal?.show();
	}

	$effect(() => {
		if (ownerSlug) {
			patchStacksStore = organizationService.getPatchStacks(ownerSlug);
		}
	});

	// Update local organization when props change
	$effect(() => {
		localOrganization = organization;
	});
</script>

<div class="org-landing-page">
	<div class="header-section">
		<div class="org-header">
			{#if localOrganization.avatarUrl}
				<img
					src={localOrganization.avatarUrl}
					alt="{localOrganization.name}'s logo"
					class="avatar"
				/>
			{/if}
			<div class="org-title">
				<h1>{localOrganization.name}</h1>
				{#if localOrganization.description}
					<p class="description">{localOrganization.description}</p>
				{/if}
			</div>
		</div>
	</div>

	<div class="content-columns">
		<div class="main-column">
			<!-- Projects Section -->
			{#if localOrganization.projects && localOrganization.projects.length > 0}
				<ProjectsSection projects={localOrganization.projects} {ownerSlug} />
			{/if}

			<!-- Reviews Section -->
			<ReviewsSection reviews={patchStacksData} status={patchStacks.status} />
		</div>

		<div class="side-column">
			{#if localOrganization.inviteCode}
				<div class="section-card invite-code-section">
					<h2 class="section-title">Invite Code</h2>
					<div class="invite-link-wrapper">
						<InviteLink organizationSlug={ownerSlug} inviteCode={localOrganization.inviteCode} />
					</div>
				</div>
			{/if}

			{#if localOrganization.members && localOrganization.members.length > 0}
				<div class="section-card members-section">
					<h2 class="section-title">Members</h2>
					<div class="members-list">
						{#each localOrganization.members as member}
							<div class="member-card">
								<a href="/{member.login}" class="member-link">
									<img
										src={member.avatar_url || '/images/default-avatar.png'}
										alt="{member.login}'s avatar"
										class="member-avatar"
									/>
									<div class="member-info">
										<span class="member-login">{member.login}</span>
										<span class="member-role">{member.name}</span>
									</div>
								</a>

								{#if localOrganization.inviteCode || member.login === currentUserLogin}
									<Button
										kind="outline"
										style="error"
										onclick={() => confirmRemoveUserDialog(member.login)}
										class="remove-button"
									>
										Remove
									</Button>
								{/if}
							</div>
						{/each}
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>

<!-- Confirmation Modal -->
<Modal bind:this={confirmRemoveUserModal} width="small" onSubmit={removeUser}>
	<p>Are you sure you want to remove this user from the organization?</p>
	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="error" type="submit" loading={isRemoving}>Remove</Button>
	{/snippet}
</Modal>

<style>
	.org-landing-page {
		color: #333;
	}

	.header-section {
		margin: 10px 0;
	}

	.org-header {
		display: flex;
		align-items: center;
		gap: 1.5rem;
		margin-bottom: 1.5rem;
	}

	.avatar {
		width: 120px;
		height: 120px;
		border-radius: 12px;
		object-fit: cover;
		box-shadow: 0 4px 8px rgba(0, 0, 0, 0.1);
	}

	.org-title {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 20px;
	}

	.org-title .description {
		font-size: 14px;
		color: #666;
	}

	h1 {
		font-size: 30px;
		margin: 0;
		line-height: 10px;
		color: #1a202c;
	}

	.content-columns {
		display: grid;
		grid-template-columns: 3fr 1fr;
		gap: 2rem;
	}

	.section-card {
		background-color: white;
		border-radius: 8px;
		margin-bottom: 2rem;
		overflow: hidden;
		border: 1px solid color(srgb 0.831373 0.815686 0.807843);
	}

	.section-title {
		font-size: 0.8em;
		margin: 0;
		padding: 12px 15px;
		border-bottom: 1px solid color(srgb 0.831373 0.815686 0.807843);
		background-color: #f3f3f2;
		color: color(srgb 0.52549 0.494118 0.47451);
	}

	.member-card {
		padding: 0.75rem;
		border-bottom: 1px solid #e2e8f0;
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.member-card:last-child {
		border-bottom: none;
	}

	.member-link {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		color: inherit;
		text-decoration: none;
	}

	.member-avatar {
		width: 40px;
		height: 40px;
		border-radius: 50%;
		object-fit: cover;
	}

	.member-info {
		display: flex;
		flex-direction: column;
	}

	.member-login {
		font-weight: 500;
		color: #2d3748;
	}

	.member-role {
		font-size: 0.8rem;
		color: #718096;
	}

	@media (max-width: 768px) {
		.content-columns {
			grid-template-columns: 1fr;
		}

		.org-header {
			flex-direction: column;
			align-items: flex-start;
			text-align: center;
		}

		.avatar {
			margin: 0 auto;
		}

		.org-title {
			align-items: center;
		}
	}
</style>

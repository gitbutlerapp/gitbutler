<script lang="ts">
	import InviteLink from '$lib/components/InviteLink.svelte';
	import OrganizationEditModal from '$lib/components/OrganizationEditModal.svelte';
	import ProjectsSection from '$lib/components/ProjectsSection.svelte';
	import ReviewsSection from '$lib/components/ReviewsSection.svelte';
	import { OWNER_SERVICE } from '$lib/owner/ownerService';
	import { UserService, USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import { ORGANIZATION_SERVICE } from '@gitbutler/shared/organizations/organizationService';
	import { Button, Modal } from '@gitbutler/ui';

	import type { ExtendedOrganization, OrganizationMember } from '$lib/owner/types';

	interface Props {
		organization: ExtendedOrganization;
		ownerSlug: string;
	}

	let { organization, ownerSlug }: Props = $props();

	// Get organization service from context
	const organizationService = inject(ORGANIZATION_SERVICE);

	// Get user service from context
	const userService = inject(USER_SERVICE) as UserService;

	// Get owner service to refresh organization data
	const ownerService = inject(OWNER_SERVICE);

	let patchStacksStore = $state(organizationService.getPatchStacks(ownerSlug));
	let patchStacks = $derived($patchStacksStore);
	let patchStacksData = $derived(patchStacks.status === 'found' ? patchStacks.value || [] : []);

	// Create a local mutable copy of the organization for updates
	let localOrganization = $derived(organization);

	// Get current user information
	let currentUser = $state(userService.user);
	let currentUserLogin = $derived($currentUser?.login);

	// Role constants
	const ROLE_OWNER = 'owner';

	// Modals for confirmation
	let confirmRemoveUserModal = $state<Modal>();
	let confirmMakeOwnerModal = $state<Modal>();
	let organizationEditModal = $state<ReturnType<typeof OrganizationEditModal>>();
	let userToRemove = $state<string | undefined>(undefined);
	let userToPromote = $state<string | undefined>(undefined);
	let isRemoving = $state(false);
	let isPromoting = $state(false);

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

	// Function to check if current user is an admin/owner of this organization
	function currentUserIsAdmin(): boolean {
		if (!currentUserLogin || !localOrganization.members) return false;

		const currentMember = localOrganization.members.find(
			(member) => member.login === currentUserLogin
		);
		return currentMember ? isOwner(currentMember) : false;
	}

	// Function to handle organization update from edit modal
	async function handleOrganizationUpdate(newSlug: string) {
		// Refresh the organization data
		if (newSlug === ownerSlug) {
			// If slug didn't change, just refresh the data
			await refreshOrganizationData();
		}
		// If slug changed, the page will be redirected by the edit modal
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
			userToRemove = undefined;
			confirmRemoveUserModal?.close();
		}
	}

	// Function to handle making a user an owner
	async function makeUserOwner() {
		if (!userToPromote || !ownerSlug) return;

		isPromoting = true;
		try {
			await organizationService.changeUserRole(ownerSlug, userToPromote, ROLE_OWNER);

			// Update the user's role locally for immediate UI feedback
			if (localOrganization.members) {
				localOrganization.members = localOrganization.members.map((member) => {
					if (member.login === userToPromote) {
						return { ...member, role: ROLE_OWNER };
					}
					return member;
				});
			}

			// Refresh data from server to ensure consistency
			await refreshOrganizationData();
		} catch (error) {
			console.error('Failed to make user an owner:', error);
		} finally {
			isPromoting = false;
			userToPromote = undefined;
			confirmMakeOwnerModal?.close();
		}
	}

	// Function to open removal confirmation modal
	function confirmRemoveUserDialog(login: string) {
		userToRemove = login;
		confirmRemoveUserModal?.show();
	}

	// Function to open make owner confirmation modal
	function confirmMakeOwnerDialog(login: string) {
		userToPromote = login;
		confirmMakeOwnerModal?.show();
	}

	// Helper function to check if a member is an owner
	function isOwner(member: OrganizationMember) {
		return member.role === ROLE_OWNER;
	}

	$effect(() => {
		if (ownerSlug) {
			patchStacksStore = organizationService.getPatchStacks(ownerSlug);
		}
	});

	// Update local organization when props change
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

			{#if currentUserIsAdmin()}
				<div class="org-actions">
					<Button style="pop" onclick={() => organizationEditModal?.show()}>
						Edit Organization
					</Button>
				</div>
			{/if}
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
										<span class="member-role">
											{member.name}
											{#if isOwner(member)}
												<span class="badge owner-badge">Owner</span>
											{/if}
										</span>
									</div>
								</a>

								<div class="member-actions">
									{#if localOrganization.inviteCode && !isOwner(member)}
										<Button
											kind="outline"
											style="neutral"
											onclick={() => confirmMakeOwnerDialog(member.login)}
										>
											Make Owner
										</Button>
									{/if}

									{#if localOrganization.inviteCode || member.login === currentUserLogin}
										<Button
											kind="outline"
											style="error"
											onclick={() => confirmRemoveUserDialog(member.login)}
										>
											Remove
										</Button>
									{/if}
								</div>
							</div>
						{/each}
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>

<!-- Remove User Confirmation Modal -->
<Modal bind:this={confirmRemoveUserModal} width="small" onSubmit={removeUser}>
	<p>Are you sure you want to remove this user from the organization?</p>
	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="error" type="submit" loading={isRemoving}>Remove</Button>
	{/snippet}
</Modal>

<!-- Make Owner Confirmation Modal -->
<Modal bind:this={confirmMakeOwnerModal} width="small" onSubmit={makeUserOwner}>
	<p>Are you sure you want to make this user an owner?</p>
	<p class="modal-note">
		Owners have full administrative access to the organization, including managing members,
		projects, and settings.
	</p>
	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit" loading={isPromoting}>Make Owner</Button>
	{/snippet}
</Modal>

<!-- Organization Edit Modal -->
<OrganizationEditModal
	bind:this={organizationEditModal}
	organizationSlug={ownerSlug}
	onUpdate={handleOrganizationUpdate}
/>

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
		width: 100%;
		margin-bottom: 1.5rem;
		gap: 1.5rem;
	}

	.org-actions {
		margin-left: auto;
	}

	.avatar {
		width: 120px;
		height: 120px;
		object-fit: cover;
		border-radius: 12px;
		box-shadow: 0 4px 8px rgba(0, 0, 0, 0.1);
	}

	.org-title {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 20px;
	}

	.org-title .description {
		color: #666;
		font-size: 14px;
	}

	h1 {
		margin: 0;
		color: #1a202c;
		font-size: 30px;
		line-height: 10px;
	}

	.content-columns {
		display: grid;
		grid-template-columns: 3fr 1fr;
		gap: 2rem;
	}

	.section-card {
		margin-bottom: 2rem;
		overflow: hidden;
		border: 1px solid color(srgb 0.831373 0.815686 0.807843);
		border-radius: 8px;
		background-color: white;
	}

	.section-title {
		margin: 0;
		padding: 12px 15px;
		border-bottom: 1px solid color(srgb 0.831373 0.815686 0.807843);
		background-color: #f3f3f2;
		color: color(srgb 0.52549 0.494118 0.47451);
		font-size: 0.8em;
	}

	.member-card {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem;
		border-bottom: 1px solid #e2e8f0;
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
		object-fit: cover;
		border-radius: 50%;
	}

	.member-info {
		display: flex;
		flex-direction: column;
	}

	.member-login {
		color: #2d3748;
		font-weight: 500;
	}

	.member-role {
		color: #718096;
		font-size: 0.8rem;
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

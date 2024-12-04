<script lang="ts">
	import { getContext } from '$lib/context';
	import RegisterInterest from '$lib/interest/RegisterInterest.svelte';
	import { OrganizationService } from '$lib/organizations/organizationService';
	import { organizationsSelectors } from '$lib/organizations/organizationsSlice';
	import { ProjectService } from '$lib/organizations/projectService';
	import { projectsSelectors } from '$lib/organizations/projectsSlice';
	import { UserService } from '$lib/users/userService';
	import { usersSelectors } from '$lib/users/usersSlice';
	import { AppState } from '$lib/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import type { Interest } from '$lib/interest/intrestStore';
	import type { Organization } from '$lib/organizations/types';
	import type { User } from '$lib/users/types';

	type Props = {
		slug: string;
	};

	const { slug }: Props = $props();

	const appState = getContext(AppState);
	const organizationService = getContext(OrganizationService);
	const projectService = getContext(ProjectService);
	const userService = getContext(UserService);

	const organizationInterest = $derived(
		organizationService.getOrganizationWithDetailsInterest(slug)
	);

	const organization = $derived<Organization | undefined>(
		organizationsSelectors.selectById(appState.organizations, slug)
	);
	const users = $derived<{ interest: Interest; user: User | undefined }[]>(
		organization?.memberLogins?.map((login) => ({
			interest: userService.getUserInterest(login),
			user: usersSelectors.selectById(appState.users, login)
		})) || []
	);

	const projects = $derived(
		organization?.projectRepositoryIds?.map((repositoryId) => ({
			project: projectsSelectors.selectById(appState.projects, repositoryId),
			projectInterest: projectService.getProjectInterest(repositoryId)
		})) || []
	);

	function onModalClose() {}

	let modal = $state<Modal>();
</script>

<Modal bind:this={modal} onClose={onModalClose} title={organization?.name ?? organization?.slug}>
	<RegisterInterest interest={organizationInterest} />

	<h5 class="text-15 text-bold">Users:</h5>
	{#if organization?.inviteCode}
		<div class="header-with-action">
			<p>Invite code:</p>
			<Textbox value={organization.inviteCode} readonly></Textbox>
		</div>
	{/if}

	<div>
		{#each users as user, index}
			<RegisterInterest interest={user.interest} />

			<SectionCard
				roundedBottom={index === users.length - 1}
				roundedTop={index === 0}
				orientation="row"
			>
				<Avatar
					size="medium"
					tooltip={user.user?.name || 'Unknown'}
					srcUrl={user.user?.avatarUrl || ''}
				/>
				<p>{user.user?.name}</p>
			</SectionCard>
		{/each}
	</div>

	<h5 class="text-15 text-bold">Projects:</h5>
	<div>
		{#each projects as { project, projectInterest }, index}
			<RegisterInterest interest={projectInterest} />

			<SectionCard
				roundedBottom={index === projects.length - 1}
				roundedTop={index === 0}
				orientation="row"
			>
				<p>{project?.name}</p>
			</SectionCard>
		{/each}
	</div>
</Modal>

<Button onclick={() => modal?.show()}>View</Button>

<style lang="postcss">
	.header-with-action {
		display: flex;
		justify-content: space-between;
		align-items: center;

		margin-bottom: 8px;
	}
</style>

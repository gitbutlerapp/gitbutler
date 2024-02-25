<script lang="ts">
	import { getCloudApiClient, type User } from '$lib/backend/cloud';
	import ClickableCard from '$lib/components/ClickableCard.svelte';
	import Link from '$lib/components/Link.svelte';
	import Spacer from '$lib/components/Spacer.svelte';
	import Toggle from '$lib/components/Toggle.svelte';
	import WelcomeSigninAction from '$lib/components/WelcomeSigninAction.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { projectAiGenAutoBranchNamingEnabled } from '$lib/config/config';
	import * as toasts from '$lib/utils/toasts';
	import { createEventDispatcher, onMount } from 'svelte';
	import type { Project } from '$lib/backend/projects';
	import type { UserService } from '$lib/stores/user';
	import { PUBLIC_API_BASE_URL } from '$env/static/public';

	export let project: Project;
	export let user: User | undefined;
	export let userService: UserService;

	const cloud = getCloudApiClient();
	const aiGenEnabled = projectAiGenEnabled(project.id);
	const aiGenAutoBranchNamingEnabled = projectAiGenAutoBranchNamingEnabled(project.id);

	const dispatch = createEventDispatcher<{
		updated: Project;
	}>();

	onMount(async () => {
		if (!project?.api) return;
		if (!user) return;
		const cloudProject = await cloud.projects.get(user.access_token, project.api.repository_id);
		if (cloudProject === project.api) return;
		dispatch('updated', { ...project, api: { ...cloudProject, sync: project.api.sync } });
	});

	const onSyncChange = async (event: CustomEvent<boolean>) => {
		if (!user) return;
		try {
			const cloudProject =
				project.api ??
				(await cloud.projects.create(user.access_token, {
					name: project.title,
					description: project.description,
					uid: project.id
				}));
			dispatch('updated', { ...project, api: { ...cloudProject, sync: event.detail } });
		} catch (error) {
			console.error(`Failed to update project sync status: ${error}`);
			toasts.error('Failed to update project sync status');
		}
	};

	const aiGenToggle = () => {
		$aiGenEnabled = !$aiGenEnabled;
		$aiGenAutoBranchNamingEnabled = $aiGenEnabled;
	};

	const aiGenBranchNamesToggle = () => {
		$aiGenAutoBranchNamingEnabled = !$aiGenAutoBranchNamingEnabled;
	};
</script>

{#if user}
	<div class="aigen-wrap">
		<ClickableCard on:click={aiGenToggle}>
			<svelte:fragment slot="title">Enable branch and commit message generation</svelte:fragment>
			<svelte:fragment slot="body">
				Uses OpenAI's API. If enabled, diffs will sent to OpenAI's servers when pressing the
				"Generate message" button.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle checked={$aiGenEnabled} on:change={aiGenToggle} />
			</svelte:fragment>
		</ClickableCard>

		<ClickableCard disabled={!$aiGenEnabled} on:click={aiGenBranchNamesToggle}>
			<svelte:fragment slot="title">Automatically generate branch names</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					disabled={!$aiGenEnabled}
					checked={$aiGenAutoBranchNamingEnabled}
					on:change={aiGenBranchNamesToggle}
				/>
			</svelte:fragment>
		</ClickableCard>
	</div>

	<Spacer />

	{#if user.role === 'admin'}
		<h3 class="text-base-15 text-bold">Full data synchronization</h3>

		<ClickableCard on:change={onSyncChange}>
			<svelte:fragment slot="body">
				Sync my history, repository and branch data for backup, sharing and team features.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle checked={project.api?.sync || false} on:change={onSyncChange} />
			</svelte:fragment>
		</ClickableCard>

		{#if project.api}
			<div class="api-link">
				<Link
					target="_blank"
					rel="noreferrer"
					href="{PUBLIC_API_BASE_URL}projects/{project.api?.repository_id}"
					>Go to GitButler Cloud Project</Link
				>
			</div>
		{/if}
		<Spacer />
	{/if}
{:else}
	<WelcomeSigninAction {userService} />
	<Spacer />
{/if}

<style lang="post-css">
	.aigen-wrap {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	.api-link {
		display: flex;
		justify-content: flex-end;
	}
</style>

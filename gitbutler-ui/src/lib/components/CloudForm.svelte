<script lang="ts">
	import SectionCard from './SectionCard.svelte';
	import { getCloudApiClient } from '$lib/backend/cloud';
	import Link from '$lib/components/Link.svelte';
	import Spacer from '$lib/components/Spacer.svelte';
	import Toggle from '$lib/components/Toggle.svelte';
	import WelcomeSigninAction from '$lib/components/WelcomeSigninAction.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { projectAiGenAutoBranchNamingEnabled } from '$lib/config/config';
	import { UserService } from '$lib/stores/user';
	import { getContextByClass } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { createEventDispatcher, onMount } from 'svelte';
	import type { Project } from '$lib/backend/projects';
	import { PUBLIC_API_BASE_URL } from '$env/static/public';

	export let project: Project;

	const userService = getContextByClass(UserService);
	const user = userService.user;

	const cloud = getCloudApiClient();
	const aiGenEnabled = projectAiGenEnabled(project.id);
	const aiGenAutoBranchNamingEnabled = projectAiGenAutoBranchNamingEnabled(project.id);

	const dispatch = createEventDispatcher<{
		updated: Project;
	}>();

	onMount(async () => {
		if (!project?.api) return;
		if (!$user) return;
		const cloudProject = await cloud.projects.get($user.access_token, project.api.repository_id);
		if (cloudProject === project.api) return;
		dispatch('updated', { ...project, api: { ...cloudProject, sync: project.api.sync } });
	});

	async function onSyncChange(sync: boolean) {
		if (!$user) return;
		try {
			const cloudProject =
				project.api ??
				(await cloud.projects.create($user.access_token, {
					name: project.title,
					description: project.description,
					uid: project.id
				}));
			dispatch('updated', { ...project, api: { ...cloudProject, sync } });
		} catch (error) {
			console.error(`Failed to update project sync status: ${error}`);
			toasts.error('Failed to update project sync status');
		}
	}

	function aiGenToggle() {
		$aiGenEnabled = !$aiGenEnabled;
		$aiGenAutoBranchNamingEnabled = $aiGenEnabled;
	}

	function aiGenBranchNamesToggle() {
		$aiGenAutoBranchNamingEnabled = !$aiGenAutoBranchNamingEnabled;
	}
</script>

{#if $user}
	<div class="aigen-wrap">
		<SectionCard labelFor="aiGenEnabled" on:click={aiGenToggle} orientation="row">
			<svelte:fragment slot="title">Enable branch and commit message generation</svelte:fragment>
			<svelte:fragment slot="caption">
				Uses OpenAI's API. If enabled, diffs will sent to OpenAI's servers when pressing the
				"Generate message" button.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle id="aiGenEnabled" checked={$aiGenEnabled} on:change={aiGenToggle} />
			</svelte:fragment>
		</SectionCard>

		<SectionCard
			labelFor="branchNameGen"
			disabled={!$aiGenEnabled}
			on:click={aiGenBranchNamesToggle}
			orientation="row"
		>
			<svelte:fragment slot="title">Automatically generate branch names</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="branchNameGen"
					disabled={!$aiGenEnabled}
					checked={$aiGenAutoBranchNamingEnabled}
					on:change={aiGenBranchNamesToggle}
				/>
			</svelte:fragment>
		</SectionCard>
	</div>

	<Spacer />

	{#if $user.role === 'admin'}
		<h3 class="text-base-15 text-bold">Full data synchronization</h3>

		<SectionCard labelFor="historySync" on:change={(e) => onSyncChange(e.detail)} orientation="row">
			<svelte:fragment slot="caption">
				Sync my history, repository and branch data for backup, sharing and team features.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="historySync"
					checked={project.api?.sync || false}
					on:change={(e) => onSyncChange(e.detail)}
				/>
			</svelte:fragment>
		</SectionCard>

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
	<WelcomeSigninAction />
	<Spacer />
{/if}

<style lang="post-css">
	.aigen-wrap {
		display: flex;
		flex-direction: column;
		gap: var(--size-4);
	}

	.api-link {
		display: flex;
		justify-content: flex-end;
	}
</style>

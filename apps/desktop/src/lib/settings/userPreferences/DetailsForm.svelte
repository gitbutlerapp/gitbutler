<script lang="ts">
	import { Project, ProjectService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Section from '$lib/settings/Section.svelte';
	import Link from '$lib/shared/Link.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import TextArea from '$lib/shared/TextArea.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { User } from '$lib/stores/user';
	import * as toasts from '$lib/utils/toasts';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import { PUBLIC_API_BASE_URL } from '$env/static/public';

	const project = getContext(Project);
	const user = getContextStore(User);
	const projectService = getContext(ProjectService);

	let title = project?.title;
	let description = project?.description;

	async function saveProject() {
		const api =
			$user && project.api
				? await projectService.updateCloudProject(project.api.repository_id, {
						name: project.title,
						description: project.description
					})
				: undefined;
		project.api = api ? { ...api, sync: false, sync_code: undefined } : undefined;
		projectService.updateProject(project);
	}

	async function onSyncChange(sync: boolean) {
		if (!$user) return;
		try {
			const cloudProject =
				project.api ??
				(await projectService.createCloudProject({
					name: project.title,
					description: project.description,
					uid: project.id
				}));
			project.api = { ...cloudProject, sync, sync_code: project.api?.sync_code };
			projectService.updateProject(project);
		} catch (error) {
			console.error(`Failed to update project sync status: ${error}`);
			toasts.error('Failed to update project sync status');
		}
	}
	// These functions are disgusting
	async function onSyncCodeChange(sync_code: boolean) {
		if (!$user) return;
		try {
			const cloudProject =
				project.api ??
				(await projectService.createCloudProject({
					name: project.title,
					description: project.description,
					uid: project.id
				}));
			project.api = { ...cloudProject, sync: project.api?.sync || false, sync_code: sync_code };
			projectService.updateProject(project);
		} catch (error) {
			console.error(`Failed to update project sync status: ${error}`);
			toasts.error('Failed to update project sync status');
		}
	}
</script>

<SectionCard>
	<form>
		<fieldset class="fields-wrapper">
			<TextBox label="Project path" readonly id="path" value={project?.path} />
			<section class="description-wrapper">
				<TextBox
					label="Project name"
					id="name"
					placeholder="Project name can't be empty"
					bind:value={title}
					required
					on:change={(e) => {
						project.title = e.detail;
						saveProject();
					}}
				/>
				<TextArea
					id="description"
					rows={3}
					placeholder="Project description"
					bind:value={description}
					on:change={() => {
						project.description = description;
						saveProject();
					}}
					maxHeight={300}
				/>
			</section>
		</fieldset>
	</form>
</SectionCard>

{#if $user?.role === 'admin'}
	<Spacer />
	<Section>
		<svelte:fragment slot="title">Full data synchronization</svelte:fragment>

		<SectionCard labelFor="historySync" orientation="row">
			<svelte:fragment slot="caption">
				Sync this project's operations log with GitButler Web services. The operations log includes
				snapshots of the repository state, including non-committed code changes.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="historySync"
					checked={project.api?.sync || false}
					on:click={async (e) => await onSyncChange(!!e.detail)}
				/>
			</svelte:fragment>
		</SectionCard>
		<SectionCard labelFor="branchesySync" orientation="row">
			<svelte:fragment slot="caption">
				Sync this repository's branches with the GitButler Remote.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="branchesySync"
					checked={project.api?.sync_code || false}
					on:click={async (e) => await onSyncCodeChange(!!e.detail)}
				/>
			</svelte:fragment>
		</SectionCard>

		{#if project.api}
			<div class="api-link text-12">
				<Link
					target="_blank"
					rel="noreferrer"
					href="{PUBLIC_API_BASE_URL}projects/{project.api?.repository_id}"
					>Go to GitButler Cloud Project</Link
				>
			</div>
		{/if}
	</Section>
{/if}
<Spacer />

<style>
	.fields-wrapper {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.description-wrapper {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.api-link {
		display: flex;
		justify-content: flex-end;
	}
</style>

<script lang="ts">
	import { Project, ProjectsService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Section from '$lib/settings/Section.svelte';
	import Link from '$lib/shared/Link.svelte';
	import { User } from '$lib/stores/user';
	import * as toasts from '$lib/utils/toasts';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import { onMount } from 'svelte';
	import { PUBLIC_API_BASE_URL } from '$env/static/public';

	const project = getContext(Project);
	const user = getContextStore(User);
	const projectsService = getContext(ProjectsService);

	let title = $state(project?.title);
	let description = $state(project?.description);

	async function onSyncChange(sync: boolean) {
		if (!$user) return;
		try {
			const cloudProject =
				project.api ??
				(await projectsService.createCloudProject({
					name: project.title,
					description: project.description,
					uid: project.id
				}));
			project.api = { ...cloudProject, sync, sync_code: project.api?.sync_code };
			projectsService.updateProject(project);
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
				(await projectsService.createCloudProject({
					name: project.title,
					description: project.description,
					uid: project.id
				}));
			project.api = { ...cloudProject, sync: project.api?.sync || false, sync_code: sync_code };
			projectsService.updateProject(project);
		} catch (error) {
			console.error(`Failed to update project sync status: ${error}`);
			toasts.error('Failed to update project sync status');
		}
	}

	// This is some janky bullshit, but it works well enough for now
	onMount(async () => {
		if (!project?.api) return;
		if (!$user) return;
		const cloudProject = await projectsService.getCloudProject(project.api.repository_id);
		project.api = { ...cloudProject, sync: project.api.sync, sync_code: project.api.sync_code };
		projectsService.updateProject(project);
	});

	$effect(() => {
		if (description) {
			console.log('description', description);
		}
	});
</script>

<SectionCard>
	<form>
		<fieldset class="fields-wrapper">
			<Textbox label="Project path" readonly id="path" value={project?.path} />
			<section class="description-wrapper">
				<Textbox
					label="Project name"
					id="name"
					placeholder="Project name can't be empty"
					bind:value={title}
					required
					onchange={(value: string) => {
						project.title = value;
						projectsService.updateProject(project);
					}}
				/>
				<Textarea
					id="description"
					minRows={3}
					maxRows={6}
					placeholder="Project description"
					bind:value={description}
					oninput={(e: Event) => {
						const target = e.currentTarget as HTMLTextAreaElement;
						project.description = target.value;
						projectsService.updateProject(project);
					}}
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
					onclick={async () => await onSyncChange(!project.api?.sync)}
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
					onclick={async () => await onSyncCodeChange(!project.api?.sync_code)}
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

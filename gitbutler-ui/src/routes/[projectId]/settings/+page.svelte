<script lang="ts">
	import CloudForm from '$lib/components/CloudForm.svelte';
	import DetailsForm from '$lib/components/DetailsForm.svelte';
	import FullscreenLoading from '$lib/components/FullscreenLoading.svelte';
	import KeysForm from '$lib/components/KeysForm.svelte';
	import PreferencesForm from '$lib/components/PreferencesForm.svelte';
	import RemoveProjectButton from '$lib/components/RemoveProjectButton.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Spacer from '$lib/components/Spacer.svelte';
	import ContentWrapper from '$lib/components/settings/ContentWrapper.svelte';
	import * as toasts from '$lib/utils/toasts';
	import type { Project } from '$lib/backend/projects';
	import type { PageData } from './$types';
	import { goto } from '$app/navigation';

	export let data: PageData;

	$: projectService = data.projectService;
	$: project$ = data.project$;
	$: userService = data.userService;
	$: user$ = data.user$;
	$: cloud = data.cloud;
	$: authService = data.authService;
	$: baseBranchService = data.baseBranchService;

	let deleteConfirmationModal: RemoveProjectButton;
	let isDeleting = false;

	async function onDeleteClicked() {
		isDeleting = true;
		try {
			projectService.deleteProject($project$?.id);
			toasts.success('Project deleted');
			goto('/');
		} catch (err: any) {
			console.error(err);
			toasts.error('Failed to delete project');
		} finally {
			isDeleting = false;
			projectService.reload();
		}
	}

	async function onCloudUpdated(e: { detail: Project }) {
		projectService.updateProject({ ...$project$, ...e.detail });
	}

	async function onPreferencesUpdated(e: {
		detail: { ok_with_force_push?: boolean; omit_certificate_check?: boolean };
	}) {
		await projectService.updateProject({ ...$project$, ...e.detail });
	}

	async function onDetailsUpdated(e: { detail: Project }) {
		const api =
			$user$ && e.detail.api
				? await cloud.projects.update($user$?.access_token, e.detail.api.repository_id, {
						name: e.detail.title,
						description: e.detail.description
					})
				: undefined;
		projectService.updateProject({
			...e.detail,
			api: api ? { ...api, sync: e.detail.api?.sync || false } : undefined
		});
	}
</script>

{#if !$project$}
	<FullscreenLoading />
{:else}
	<ContentWrapper title="Project settings">
		<CloudForm project={$project$} user={$user$} {userService} on:updated={onCloudUpdated} />
		<DetailsForm project={$project$} on:updated={onDetailsUpdated} />
		<KeysForm project={$project$} {authService} {baseBranchService} {projectService} />
		<Spacer />
		<PreferencesForm project={$project$} on:updated={onPreferencesUpdated} />
		<SectionCard>
			<svelte:fragment slot="title">Remove project</svelte:fragment>
			<svelte:fragment slot="body">
				You can remove projects from GitButler, your code remains safe as this only clears
				configuration.
			</svelte:fragment>
			<div>
				<RemoveProjectButton
					bind:this={deleteConfirmationModal}
					projectTitle={$project$?.title}
					{isDeleting}
					{onDeleteClicked}
				/>
			</div>
		</SectionCard>
	</ContentWrapper>
{/if}

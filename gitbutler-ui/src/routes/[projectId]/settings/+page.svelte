<script lang="ts">
	import CloudForm from '$lib/components/CloudForm.svelte';
	import DetailsForm from '$lib/components/DetailsForm.svelte';
	import FullscreenLoading from '$lib/components/FullscreenLoading.svelte';
	import KeysForm from '$lib/components/KeysForm.svelte';
	import PreferencesForm from '$lib/components/PreferencesForm.svelte';
	import RemoveProjectButton from '$lib/components/RemoveProjectButton.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import ContentWrapper from '$lib/components/settings/ContentWrapper.svelte';
	import * as toasts from '$lib/utils/toasts';
	import type { UserError } from '$lib/backend/ipc';
	import type { Key, Project } from '$lib/backend/projects';
	import type { PageData } from './$types';
	import { goto } from '$app/navigation';

	export let data: PageData;

	$: projectService = data.projectService;
	$: project$ = data.project$;
	$: userService = data.userService;
	$: user$ = data.user$;
	$: cloud = data.cloud;

	let deleteConfirmationModal: RemoveProjectButton;
	let isDeleting = false;

	const onDeleteClicked = () =>
		Promise.resolve()
			.then(() => (isDeleting = true))
			.then(() => projectService.deleteProject($project$?.id))
			.catch((e) => {
				console.error(e);
				toasts.error('Failed to delete project');
			})
			.then(() => toasts.success('Project deleted'))
			.then(() => goto('/'))
			.finally(() => {
				isDeleting = false;
				projectService.reload();
			});

	const onKeysUpdated = (e: { detail: { preferred_key: Key } }) =>
		projectService
			.updateProject({ ...$project$, ...e.detail })
			.then(() => toasts.success('Preferred key updated'))
			.catch((e: UserError) => {
				toasts.error(e.message);
			});
	const onCloudUpdated = (e: { detail: Project }) =>
		projectService.updateProject({ ...$project$, ...e.detail });
	const onPreferencesUpdated = (e: {
		detail: { ok_with_force_push?: boolean; omit_certificate_check?: boolean };
	}) => projectService.updateProject({ ...$project$, ...e.detail });
	const onDetailsUpdated = async (e: { detail: Project }) => {
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
	};
</script>

{#if !$project$}
	<FullscreenLoading />
{:else}
	<ContentWrapper title="Project settings">
		<CloudForm project={$project$} user={$user$} {userService} on:updated={onCloudUpdated} />
		<DetailsForm project={$project$} on:updated={onDetailsUpdated} />
		<KeysForm project={$project$} on:updated={onKeysUpdated} />
		<PreferencesForm project={$project$} on:updated={onPreferencesUpdated} />
		<SectionCard>
			<svelte:fragment slot="title">Remove all projects</svelte:fragment>
			<svelte:fragment slot="body">
				You can delete all projects from the GitButler app. Your code remains safe.
				<br />
				it only clears the configuration.
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

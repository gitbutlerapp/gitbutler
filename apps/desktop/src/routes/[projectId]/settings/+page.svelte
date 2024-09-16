<script lang="ts">
	import { Project, ProjectService } from '$lib/backend/projects';
	import BaseBranchSwitch from '$lib/components/BaseBranchSwitch.svelte';
	import RemoveProjectButton from '$lib/components/RemoveProjectButton.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { featureBaseBranchSwitching } from '$lib/config/uiFeatureFlags';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { platformName } from '$lib/platform/platform';
	import CloudForm from '$lib/settings/CloudForm.svelte';
	import DetailsForm from '$lib/settings/DetailsForm.svelte';
	import KeysForm from '$lib/settings/KeysForm.svelte';
	import PreferencesForm from '$lib/settings/PreferencesForm.svelte';
	import PullRequestTemplateForm from '$lib/settings/PullRequestTemplateForm.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import { getContext } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { goto } from '$app/navigation';

	const baseBranchSwitching = featureBaseBranchSwitching();
	const projectService = getContext(ProjectService);
	const project = getContext(Project);
	const gitHost = getGitHost();

	let deleteConfirmationModal: RemoveProjectButton;
	let isDeleting = $state(false);

	async function onDeleteClicked() {
		isDeleting = true;
		try {
			await projectService.deleteProject(project.id);
			await projectService.reload();
			goto('/');
			toasts.success('Project deleted');
		} catch (err: any) {
			console.error(err);
			showError('Failed to delete project', err);
		} finally {
			isDeleting = false;
		}
	}
</script>

<SettingsPage title="Project settings">
	{#if $baseBranchSwitching}
		<BaseBranchSwitch />
	{/if}
	<CloudForm />
	<DetailsForm />
	{#if $gitHost}
		<PullRequestTemplateForm />
	{/if}
	{#if $platformName !== 'win32'}
		<KeysForm showProjectName={false} />
		<Spacer />
	{/if}
	<PreferencesForm />
	<SectionCard>
		<svelte:fragment slot="title">Remove project</svelte:fragment>
		<svelte:fragment slot="caption">
			You can remove projects from GitButler, your code remains safe as this only clears
			configuration.
		</svelte:fragment>
		<div>
			<RemoveProjectButton
				bind:this={deleteConfirmationModal}
				projectTitle={project.title}
				{isDeleting}
				{onDeleteClicked}
			/>
		</div>
	</SectionCard>
</SettingsPage>

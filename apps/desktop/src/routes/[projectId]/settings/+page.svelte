<script lang="ts">
	import { Project, ProjectService } from '$lib/backend/projects';
	import RemoveProjectButton from '$lib/components/RemoveProjectButton.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import TabContent from '$lib/components/tabs/TabContent.svelte';
	import TabList from '$lib/components/tabs/TabList.svelte';
	import TabTrigger from '$lib/components/tabs/TabTrigger.svelte';
	import Tabs from '$lib/components/tabs/Tabs.svelte';
	import { featureBaseBranchSwitching } from '$lib/config/uiFeatureFlags';
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { platformName } from '$lib/platform/platform';
	import KeysForm from '$lib/settings/KeysForm.svelte';
	import BaseBranchSwitch from '$lib/settings/userPreferences/BaseBranchSwitch.svelte';
	import CloudForm from '$lib/settings/userPreferences/CloudForm.svelte';
	import CommitSigningForm from '$lib/settings/userPreferences/CommitSigningForm.svelte';
	import DetailsForm from '$lib/settings/userPreferences/DetailsForm.svelte';
	import PreferencesForm from '$lib/settings/userPreferences/PreferencesForm.svelte';
	import { getContext } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { goto } from '$app/navigation';

	const baseBranchSwitching = featureBaseBranchSwitching();
	const projectService = getContext(ProjectService);
	const project = getContext(Project);

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
	<Tabs defaultSelected="project">
		<TabList>
			<TabTrigger value="project">Project</TabTrigger>
			<TabTrigger value="git">Git</TabTrigger>
			<TabTrigger value="ai">AI</TabTrigger>
			<TabTrigger value="feature-flags">Experimental</TabTrigger>
		</TabList>
		<TabContent value="git">
			<CommitSigningForm />
			{#if $platformName !== 'win32'}
				<KeysForm showProjectName={false} />
			{/if}
		</TabContent>
		<TabContent value="ai">
			<CloudForm />
		</TabContent>
		<TabContent value="project">
			{#if $baseBranchSwitching}
				<BaseBranchSwitch />
			{/if}
			<DetailsForm />
			<SectionCard>
				<svelte:fragment slot="title">Remove project</svelte:fragment>
				<svelte:fragment slot="caption">
					You can remove projects from GitButler, your code remains safe as this only clears
					configuration.
				</svelte:fragment>
				<div>
					<RemoveProjectButton projectTitle={project.title} {isDeleting} {onDeleteClicked} />
				</div>
			</SectionCard>
		</TabContent>
		<TabContent value="feature-flags">
			<PreferencesForm />
		</TabContent>
	</Tabs>
</SettingsPage>

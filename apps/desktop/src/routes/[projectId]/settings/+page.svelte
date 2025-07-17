<script lang="ts">
	import { page } from '$app/state';
	import BaseBranchSwitch from '$components/BaseBranchSwitch.svelte';
	import CloudForm from '$components/CloudForm.svelte';
	import DetailsForm from '$components/DetailsForm.svelte';
	import ForgeForm from '$components/ForgeForm.svelte';
	import GitForm from '$components/GitForm.svelte';
	import PreferencesForm from '$components/PreferencesForm.svelte';
	import RemoveProjectForm from '$components/RemoveProjectForm.svelte';
	import Section from '$components/Section.svelte';
	import SettingsPage from '$components/SettingsPage.svelte';
	import TabContent from '$components/TabContent.svelte';
	import TabList from '$components/TabList.svelte';
	import TabTrigger from '$components/TabTrigger.svelte';

	import Tabs from '$components/Tabs.svelte';

	// TODO: Refactor so we don't need non-null assertion.
	const projectId = $derived(page.params.projectId!);
</script>

<SettingsPage title="Project settings">
	<Tabs defaultSelected="project">
		<TabList>
			<TabTrigger value="project">Project</TabTrigger>
			<TabTrigger value="cloud">Server</TabTrigger>
			<TabTrigger value="git">Git</TabTrigger>
			<TabTrigger value="ai">AI</TabTrigger>
			<TabTrigger value="feature-flags">Experimental</TabTrigger>
		</TabList>

		<TabContent value="git">
			<GitForm {projectId} />
		</TabContent>
		<TabContent value="ai">
			<CloudForm {projectId} />
		</TabContent>
		<TabContent value="project">
			<Section>
				<DetailsForm {projectId} />
				<BaseBranchSwitch {projectId} />
				<ForgeForm {projectId} />
				<RemoveProjectForm {projectId} />
			</Section>
		</TabContent>
		<TabContent value="feature-flags">
			<PreferencesForm {projectId} />
		</TabContent>
	</Tabs>
</SettingsPage>

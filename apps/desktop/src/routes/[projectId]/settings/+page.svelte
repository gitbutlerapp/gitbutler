<script lang="ts">
	import BaseBranchSwitch from '$components/BaseBranchSwitch.svelte';
	import CloudForm from '$components/CloudForm.svelte';
	import CloudProjectSettings from '$components/CloudProjectSettings.svelte';
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
			<GitForm />
		</TabContent>
		<TabContent value="ai">
			<CloudForm />
		</TabContent>
		<TabContent value="project">
			<Section>
				<DetailsForm />
				<BaseBranchSwitch />
				<RemoveProjectForm />
				<ForgeForm />
			</Section>
		</TabContent>
		<TabContent value="cloud">
			<CloudProjectSettings />
		</TabContent>
		<TabContent value="feature-flags">
			<PreferencesForm />
		</TabContent>
	</Tabs>
</SettingsPage>

<script lang="ts">
	import TabContent from '$lib/components/tabs/TabContent.svelte';
	import TabList from '$lib/components/tabs/TabList.svelte';
	import TabTrigger from '$lib/components/tabs/TabTrigger.svelte';
	import Tabs from '$lib/components/tabs/Tabs.svelte';
	import { featureBaseBranchSwitching } from '$lib/config/uiFeatureFlags';
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
	import { platformName } from '$lib/platform/platform';
	import KeysForm from '$lib/settings/KeysForm.svelte';
	import Section from '$lib/settings/Section.svelte';
	import BaseBranchSwitch from '$lib/settings/userPreferences/BaseBranchSwitch.svelte';
	import CloudForm from '$lib/settings/userPreferences/CloudForm.svelte';
	import CommitSigningForm from '$lib/settings/userPreferences/CommitSigningForm.svelte';
	import DetailsForm from '$lib/settings/userPreferences/DetailsForm.svelte';
	import PreferencesForm from '$lib/settings/userPreferences/PreferencesForm.svelte';
	import RemoveProjectForm from '$lib/settings/userPreferences/RemoveProjectForm.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';

	const baseBranchSwitching = featureBaseBranchSwitching();
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
			<Section>
				<CommitSigningForm />
				{#if $platformName !== 'win32'}
					<Spacer />
					<KeysForm showProjectName={false} />
				{/if}
			</Section>
		</TabContent>
		<TabContent value="ai">
			<CloudForm />
		</TabContent>
		<TabContent value="project">
			<Section>
				<DetailsForm />
				{#if $baseBranchSwitching}
					<BaseBranchSwitch />
				{/if}
				<RemoveProjectForm />
			</Section>
		</TabContent>
		<TabContent value="feature-flags">
			<PreferencesForm />
		</TabContent>
	</Tabs>
</SettingsPage>

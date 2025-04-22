<script lang="ts">
	import CloudForm from '$components/CloudForm.svelte';
	import CloudProjectSettings from '$components/CloudProjectSettings.svelte';
	import GitForm from '$components/GitForm.svelte';
	import PreferencesForm from '$components/PreferencesForm.svelte';
	import SettingsPages, { type Page } from '$components/v3/SettingsPages.svelte';
	import GeneralSettings from '$components/v3/projectSettings/GeneralSettings.svelte';
	import { newProjectSettingsPath } from '$lib/routes/routes.svelte';
	import { page } from '$app/state';

	const pages: Page[] = [
		{
			id: 'project',
			label: 'Project',
			icon: 'profile',
			component: GeneralSettings
		},
		{
			id: 'cloud',
			label: 'Cloud',
			icon: 'bowtie',
			component: CloudProjectSettings
		},
		{
			id: 'git',
			label: 'Git stuff',
			icon: 'git',
			component: GitForm
		},
		{
			id: 'ai',
			label: 'AI options',
			icon: 'ai',
			component: CloudForm
		},
		{
			id: 'experimental',
			label: 'Experimental',
			icon: 'idea',
			component: PreferencesForm
		}
	];

	const projectId = $derived(page.params.projectId!);
	const selectedId = $derived(page.params.selectedId);
</script>

<SettingsPages
	title="Project settings"
	{selectedId}
	{pages}
	pageUrl={(pageId) => newProjectSettingsPath(projectId, pageId)}
	hidePageHeader
/>

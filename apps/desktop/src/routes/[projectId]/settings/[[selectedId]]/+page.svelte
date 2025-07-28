<script lang="ts">
	import CloudForm from '../CloudForm.svelte';
	import GeneralSettings from '../General.svelte';
	import GitForm from '../GitForm.svelte';
	import PreferencesForm from '../PreferencesForm.svelte';
	import { page } from '$app/state';
	import SettingsPages, { type Page } from '$components/shared/SettingsPages.svelte';
	import { newProjectSettingsPath } from '$lib/routes/routes.svelte';

	const pages: Page[] = [
		{
			type: 'project',
			id: 'project',
			label: 'Project',
			icon: 'profile',
			component: GeneralSettings
		},
		{
			type: 'project',
			id: 'git',
			label: 'Git stuff',
			icon: 'git',
			component: GitForm
		},
		{
			type: 'project',
			id: 'ai',
			label: 'AI options',
			icon: 'ai',
			component: CloudForm
		},
		{
			type: 'project',
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
	{projectId}
	{selectedId}
	{pages}
	pageUrl={(pageId) => newProjectSettingsPath(projectId, pageId)}
	hidePageHeader
/>

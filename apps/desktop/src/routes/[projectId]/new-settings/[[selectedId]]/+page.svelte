<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import CloudForm from '$components/CloudForm.svelte';
	import GitForm from '$components/GitForm.svelte';
	import PreferencesForm from '$components/PreferencesForm.svelte';
	import SettingsPages, { type Page } from '$components/SettingsPages.svelte';
	import GeneralSettings from '$components/projectSettings/GeneralSettings.svelte';
	import { newProjectSettingsPath, workspacePath } from '$lib/routes/routes.svelte';

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
	onclose={() => {
		goto(workspacePath(projectId));
	}}
	hidePageHeader
/>

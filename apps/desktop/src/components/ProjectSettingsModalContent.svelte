<script lang="ts">
	import CloudForm from '$components/CloudForm.svelte';
	import GitForm from '$components/GitForm.svelte';
	import PreferencesForm from '$components/PreferencesForm.svelte';
	import SettingsModalLayout from '$components/SettingsModalLayout.svelte';
	import AgentSettings from '$components/projectSettings/AgentSettings.svelte';
	import GeneralSettings from '$components/projectSettings/GeneralSettings.svelte';
	import { projectDisableCodegen } from '$lib/config/config';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { inject } from '@gitbutler/core/context';
	import iconsJson from '@gitbutler/ui/data/icons.json';
	import type { ProjectSettingsModalState } from '$lib/state/uiState.svelte';

	type Props = {
		data: ProjectSettingsModalState;
	};

	const { data }: Props = $props();

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;

	const codegenDisabled = $derived(projectDisableCodegen(data.projectId));

	const allPages = $derived([
		{
			id: 'project',
			label: $t('settings.project.project.label'),
			icon: 'profile' as keyof typeof iconsJson
		},
		{
			id: 'git',
			label: $t('settings.project.git.label'),
			icon: 'git' as keyof typeof iconsJson
		},
		{
			id: 'ai',
			label: $t('settings.project.ai.label'),
			icon: 'ai' as keyof typeof iconsJson
		},
		{
			id: 'agent',
			label: $t('settings.project.agent.label'),
			icon: 'ai-agent' as keyof typeof iconsJson,
			requireCodegen: true
		},
		{
			id: 'experimental',
			label: $t('settings.project.experimental.label'),
			icon: 'idea' as keyof typeof iconsJson
		}
	]);

	const pages = $derived(allPages.filter((page) => !page.requireCodegen || !$codegenDisabled));

	let currentSelectedId = $derived(data.selectedId || pages.at(0)?.id);

	function selectPage(pageId: string) {
		currentSelectedId = pageId;
	}
</script>

<SettingsModalLayout
	title={$t('settings.project.title')}
	{pages}
	selectedId={data.selectedId}
	onSelectPage={selectPage}
>
	{#snippet content({ currentPage })}
		{#if currentPage}
			{#if currentPage.id === 'project'}
				<GeneralSettings projectId={data.projectId} />
			{:else if currentPage.id === 'git'}
				<GitForm projectId={data.projectId} />
			{:else if currentPage.id === 'ai'}
				<CloudForm projectId={data.projectId} />
			{:else if currentPage.id === 'agent'}
				<AgentSettings />
			{:else if currentPage.id === 'experimental'}
				<PreferencesForm projectId={data.projectId} />
			{:else}
				{$t('settings.error.notFound', { id: currentPage.id })}
			{/if}
		{:else}
			{$t('settings.error.notFound', { id: currentSelectedId })}
		{/if}
	{/snippet}
</SettingsModalLayout>

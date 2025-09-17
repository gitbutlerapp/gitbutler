<script lang="ts">
	import CloudForm from '$components/CloudForm.svelte';
	import GitForm from '$components/GitForm.svelte';
	import PreferencesForm from '$components/PreferencesForm.svelte';
	import SettingsModalLayout from '$components/SettingsModalLayout.svelte';
	import GeneralSettings from '$components/projectSettings/GeneralSettings.svelte';
	import iconsJson from '@gitbutler/ui/data/icons.json';
	import type { ProjectSettingsModalState } from '$lib/state/uiState.svelte';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { SECRET_SERVICE } from '$lib/secrets/secretsService';
	import { GITLAB_CLIENT } from '$lib/forge/gitlab/gitlabClient.svelte';
	import { GitLabState, GITLAB_STATE } from '$lib/forge/gitlab/gitlabState.svelte';
	import { inject, provide } from '@gitbutler/core/context';

	type Props = {
		data: ProjectSettingsModalState;
	};

	const { data }: Props = $props();

	// Provide GitLab state inside the modal scope so ForgeForm can persist values
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const secretService = inject(SECRET_SERVICE);
	const gitLabClient = inject(GITLAB_CLIENT);

	const repoInfoResponse = $derived(baseBranchService.repo(data.projectId));
	const repoInfo = $derived(repoInfoResponse.current.data);

	$effect.pre(() => {
		if (!repoInfo) return;
		const gitLabState = new GitLabState(secretService, repoInfo, data.projectId);
		provide(GITLAB_STATE, gitLabState);
		gitLabClient.set(gitLabState);
	});

	const pages = [
		{
			id: 'project',
			label: 'Project',
			icon: 'profile' as keyof typeof iconsJson
		},
		{
			id: 'git',
			label: 'Git stuff',
			icon: 'git' as keyof typeof iconsJson
		},
		{
			id: 'ai',
			label: 'AI options',
			icon: 'ai' as keyof typeof iconsJson
		},
		{
			id: 'experimental',
			label: 'Experimental',
			icon: 'idea' as keyof typeof iconsJson
		}
	];

	let currentSelectedId = $state(data.selectedId || pages[0]!.id);

	function selectPage(pageId: string) {
		currentSelectedId = pageId;
	}
</script>

<SettingsModalLayout
	title="Project settings"
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
			{:else if currentPage.id === 'experimental'}
				<PreferencesForm projectId={data.projectId} />
			{:else}
				Settings page {currentPage.id} not Found.
			{/if}
		{:else}
			Settings page {currentSelectedId} not Found.
		{/if}
	{/snippet}
</SettingsModalLayout>

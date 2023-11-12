<script lang="ts">
	import { projectHttpsWarningBannerDismissed } from '$lib/config/config';
	import type { PageData } from './$types';
	import IconExternalLink from '$lib/icons/IconExternalLink.svelte';
	import Board from './Board.svelte';

	export let data: PageData;
	let {
		projectId,
		cloud,
		baseBranchStore,
		baseBranchesState,
		branchController,
		branchesWithContent,
		branchesState,
		githubContextStore,
		projectStore
	} = data;

	const httpsWarningBannerDismissed = projectHttpsWarningBannerDismissed(projectId);
	function shouldShowHttpsWarning() {
		if (httpsWarningBannerDismissed) return false;
		if (!$baseBranchStore?.remoteUrl.startsWith('https')) return false;
		if ($baseBranchStore?.remoteUrl.includes('github.com') && $githubContextStore) return false;
		return true;
	}
</script>

<div class="flex h-full w-full flex-grow flex-col overflow-hidden">
	{#if shouldShowHttpsWarning()}
		<div class="w-full bg-yellow-200/70 px-2 py-1 dark:bg-yellow-700/70">
			HTTPS remote detected. In order to push & fetch, you may need to&nbsp;
			<a class="font-bold" href="/user"> set up </a>&nbsp;an SSH key (
			<a
				target="_blank"
				rel="noreferrer"
				class="font-bold"
				href="https://docs.gitbutler.com/features/virtual-branches/pushing-and-fetching#the-ssh-keys"
			>
				docs
			</a>
			&nbsp;
			<IconExternalLink class="inline h-4 w-4" />
			).
			<button on:click={() => httpsWarningBannerDismissed.set(true)}>Dismiss</button>
		</div>
	{/if}
	<div class="flex-grow overflow-x-auto overflow-y-hidden overscroll-none">
		<Board
			{branchController}
			{projectId}
			{cloud}
			base={$baseBranchStore}
			branches={$branchesWithContent}
			projectPath={$projectStore?.path}
			cloudEnabled={$projectStore?.api?.sync || false}
			githubContext={$githubContextStore}
			branchesState={$branchesState}
			baseBranchState={$baseBranchesState}
		/>
	</div>
</div>

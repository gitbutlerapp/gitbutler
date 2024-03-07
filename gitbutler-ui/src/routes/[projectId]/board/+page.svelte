<script lang="ts">
	import Board from '$lib/components/Board.svelte';
	import IconLink from '$lib/components/IconLink.svelte';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import { projectHttpsWarningBannerDismissed } from '$lib/config/config';
	import type { PageData } from './$types';

	export let data: PageData;

	$: vbranchService = data.vbranchService;
	$: branchController = data.branchController;
	$: baseBranchService = data.baseBranchService;
	$: cloud = data.cloud;
	$: projectId = data.projectId;
	$: base$ = baseBranchService.base$;
	$: user$ = data.user$;
	$: githubService = data.githubService;
	$: branchService = data.branchService;

	$: project$ = data.project$;
	$: activeBranches$ = vbranchService.activeBranches$;
	$: error$ = vbranchService.branchesError$;
	$: githubEnabled$ = githubService.isEnabled$;

	let viewport: HTMLDivElement;
	let contents: HTMLDivElement;

	const httpsWarningBannerDismissed = projectHttpsWarningBannerDismissed(projectId);

	function shouldShowHttpsWarning() {
		if (httpsWarningBannerDismissed) return false;
		if (!$base$?.remoteUrl.startsWith('https')) return false;
		if ($base$?.remoteUrl.includes('github.com') && $githubEnabled$) return false;
		return true;
	}
</script>

<div class="flex h-full w-full max-w-full flex-grow flex-col overflow-hidden">
	{#if shouldShowHttpsWarning()}
		<div class="w-full bg-yellow-200/70 px-2 py-1 dark:bg-yellow-700/70">
			HTTPS remote detected. In order to push & fetch, you may need to&nbsp;
			<a class="font-bold" href="/settings"> set up </a>&nbsp;an SSH key (
			<IconLink
				href="https://docs.gitbutler.com/features/virtual-branches/pushing-and-fetching#the-ssh-keys"
				icon="open-link">docs</IconLink
			>
			).
			<button on:mousedown={() => httpsWarningBannerDismissed.set(true)}>Dismiss</button>
		</div>
	{/if}
	<div class="board-wrapper">
		<div class="scroll-viewport hide-native-scrollbar" bind:this={viewport}>
			<div class="scroll-contents" bind:this={contents}>
				<Board
					{branchController}
					{branchService}
					project={$project$}
					{cloud}
					base={$base$}
					branches={$activeBranches$}
					projectPath={$project$?.path}
					branchesError={$error$}
					user={$user$}
					{githubService}
				/>
			</div>
			<Scrollbar {viewport} {contents} horz />
		</div>
	</div>
</div>

<style lang="postcss">
	.scroll-viewport {
		overflow-x: scroll;
		height: 100%;
		width: 100%;
	}
	.scroll-contents {
		display: flex;
		height: 100%;
		min-width: 100%;
		width: fit-content;
	}

	/* BOARD */
	.board-wrapper {
		position: relative;
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		height: 100%;
	}
</style>

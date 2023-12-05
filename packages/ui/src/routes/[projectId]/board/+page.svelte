<script lang="ts">
	import { projectHttpsWarningBannerDismissed } from '$lib/config/config';
	import type { PageData } from './$types';
	import IconExternalLink from '$lib/icons/IconExternalLink.svelte';
	import Board from './Board.svelte';
	import Scrollbar from '$lib/components/Scrollbar.svelte';

	export let data: PageData;

	$: vbranchService = data.vbranchService;
	$: githubContext$ = data.githubContext$;
	$: branchController = data.branchController;
	$: baseBranchService = data.baseBranchService;
	$: cloud = data.cloud;
	$: projectId = data.projectId;
	$: base$ = baseBranchService.base$;
	$: user$ = data.user$;
	$: prService = data.prService;

	$: project$ = data.project$;
	$: activeBranches$ = vbranchService.activeBranches$;
	$: error$ = vbranchService.branchesError$;

	let viewport: HTMLDivElement;
	let contents: HTMLDivElement;

	const httpsWarningBannerDismissed = projectHttpsWarningBannerDismissed(projectId);

	function shouldShowHttpsWarning() {
		if (httpsWarningBannerDismissed) return false;
		if (!$base$?.remoteUrl.startsWith('https')) return false;
		if ($base$?.remoteUrl.includes('github.com') && $githubContext$) return false;
		return true;
	}
</script>

<div class="flex h-full w-full max-w-full flex-grow flex-col overflow-hidden">
	{#if shouldShowHttpsWarning()}
		<div class="w-full bg-yellow-200/70 px-2 py-1 dark:bg-yellow-700/70">
			HTTPS remote detected. In order to push & fetch, you may need to&nbsp;
			<a class="font-bold" href="/settings"> set up </a>&nbsp;an SSH key (
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
	<div class="relative h-full flex-grow overscroll-none">
		<div class="scroll-viewport hide-native-scrollbar" bind:this={viewport}>
			<div class="scroll-contents" bind:this={contents}>
				<Board
					{branchController}
					{projectId}
					{cloud}
					base={$base$}
					branches={$activeBranches$}
					projectPath={$project$?.path}
					githubContext={$githubContext$}
					branchesError={$error$}
					user={$user$}
					{prService}
				/>
			</div>
		</div>
		<Scrollbar {viewport} {contents} vertical thickness="0.4rem" />
	</div>
</div>

<style lang="postcss">
	.scroll-viewport {
		overflow-x: scroll;
		overscroll-behavior: none;
		height: 100%;
		width: 100%;
	}
	.scroll-contents {
		display: flex;
		height: 100%;
	}
</style>

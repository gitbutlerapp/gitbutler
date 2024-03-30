<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import Board from '$lib/components/Board.svelte';
	import IconLink from '$lib/components/IconLink.svelte';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import { projectHttpsWarningBannerDismissed } from '$lib/config/config';
	import { GitHubService } from '$lib/github/service';
	import { getContext } from '$lib/utils/context';
	import { BaseBranchService } from '$lib/vbranches/branchStoresCache';

	const project = getContext(Project);
	const githubService = getContext(GitHubService);
	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;

	let viewport: HTMLDivElement;
	let contents: HTMLDivElement;

	const httpsWarningBannerDismissed = projectHttpsWarningBannerDismissed(project.id);

	function shouldShowHttpsWarning() {
		if (httpsWarningBannerDismissed) return false;
		if (!$baseBranch?.remoteUrl.startsWith('https')) return false;
		if ($baseBranch?.remoteUrl.includes('github.com') && githubService.isEnabled) return false;
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
				<Board />
			</div>
			<Scrollbar {viewport} {contents} horz zIndex={50} />
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

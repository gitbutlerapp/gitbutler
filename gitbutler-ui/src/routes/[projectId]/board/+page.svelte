<script lang="ts">
	import Board from '$lib/components/Board.svelte';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import { projectHttpsWarningBannerDismissed } from '$lib/config/config';
	import { GitHubService } from '$lib/github/service';
	import { showToast } from '$lib/notifications/toasts';
	import { getContextByClass } from '$lib/utils/context';
	import { BaseBranchService } from '$lib/vbranches/branchStoresCache';
	import type { PageData } from './$types';

	export let data: PageData;

	const githubService = getContextByClass(GitHubService);
	const baseBranchService = getContextByClass(BaseBranchService);
	const baseBranch = baseBranchService.base;

	$: ({ vbranchService, projectId } = data);

	$: activeBranches$ = vbranchService.activeBranches$;
	$: error$ = vbranchService.branchesError;

	let viewport: HTMLDivElement;
	let contents: HTMLDivElement;

	const httpsWarningBannerDismissed = projectHttpsWarningBannerDismissed(projectId);

	function shouldShowHttpsWarning() {
		if (httpsWarningBannerDismissed) return false;
		if (!$baseBranch?.remoteUrl.startsWith('https')) return false;
		if ($baseBranch?.remoteUrl.includes('github.com') && githubService.isEnabled) return false;
		return true;
	}

	$: if (shouldShowHttpsWarning()) {
		showToast({
			title: 'HTTPS remote detected',
			message: 'In order to push & fetch, you may need to set up an SSH key.',
			style: 'neutral'
		});
	}
</script>

<div class="board-wrapper">
	<div id="board-viewport" class="scroll-viewport hide-native-scrollbar" bind:this={viewport}>
		<div class="scroll-contents" bind:this={contents}>
			<Board branches={$activeBranches$} branchesError={$error$} />
		</div>
		<Scrollbar {viewport} {contents} horz zIndex={50} />
	</div>
</div>

<style lang="postcss">
	/* BOARD */
	.board-wrapper {
		position: relative;
		overflow: hidden;
		height: 100%;
		width: 100%;
	}

	/* SCROLLBAR */
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
</style>

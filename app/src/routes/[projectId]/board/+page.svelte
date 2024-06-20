<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import Board from '$lib/components/Board.svelte';
	import { projectHttpsWarningBannerDismissed } from '$lib/config/config';
	import { GitHubService } from '$lib/github/service';
	import { showToast } from '$lib/notifications/toasts';
	import Scrollbar from '$lib/shared/Scrollbar.svelte';
	import { getContext } from '$lib/utils/context';
	import { BaseBranchService } from '$lib/vbranches/baseBranch';

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

	$: if (shouldShowHttpsWarning()) {
		showToast({
			title: 'HTTPS remote detected',
			message: 'In order to push & fetch, you may need to set up an SSH key.',
			style: 'neutral'
		});
	}
</script>

<div class="board-wrapper">
	<div class="scroll-viewport hide-native-scrollbar" id="board-viewport" bind:this={viewport}>
		<div class="scroll-contents" bind:this={contents}>
			<Board />
		</div>
		<Scrollbar {viewport} {contents} horz />
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
		scroll-behavior: smooth !important;
	}
	.scroll-contents {
		display: flex;
		height: 100%;
		min-width: 100%;
		width: fit-content;
	}
</style>

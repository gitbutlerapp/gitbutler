<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import Board from '$lib/components/Board.svelte';
	import { projectHttpsWarningBannerDismissed } from '$lib/config/config';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { ModeService } from '$lib/modes/service';
	import { showToast } from '$lib/notifications/toasts';
	import Scrollbar from '$lib/scroll/Scrollbar.svelte';
	import { getContext } from '$lib/utils/context';
	import { goto } from '$app/navigation';

	const project = getContext(Project);
	const gitHost = getGitHost();
	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;

	let viewport = $state<HTMLDivElement>();
	let contents = $state<HTMLDivElement>();

	const httpsWarningBannerDismissed = projectHttpsWarningBannerDismissed(project.id);

	function shouldShowHttpsWarning() {
		if (httpsWarningBannerDismissed) return false;
		if (!$baseBranch?.remoteUrl.startsWith('https')) return false;
		if ($baseBranch?.remoteUrl.includes('github.com') && !!$gitHost) return false;
		return true;
	}

	$effect(() => {
		if (shouldShowHttpsWarning()) {
			showToast({
				title: 'HTTPS remote detected',
				message: 'In order to push & fetch, you may need to set up an SSH key.',
				style: 'neutral'
			});
		}
	});

	const modeService = getContext(ModeService);
	const mode = modeService.mode;

	function gotoEdit() {
		goto(`/${project.id}/edit`);
	}

	$effect(() => {
		if ($mode?.type === 'Edit') {
			// That was causing an incorrect linting error when project.id was accessed inside the reactive block
			gotoEdit();
		}
	});
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

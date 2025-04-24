<script lang="ts">
	import Board from '$components/Board.svelte';
	import Scrollbar from '$components/Scrollbar.svelte';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { projectHttpsWarningBannerDismissed } from '$lib/config/config';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { ModeService } from '$lib/mode/modeService';
	import { showToast } from '$lib/notifications/toasts';
	import { Project } from '$lib/project/project';
	import { getContext } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';

	const project = getContext(Project);
	const forge = getContext(DefaultForgeFactory);
	const baseBranchService = getContext(BaseBranchService);
	const baseRepoResponse = $derived(baseBranchService.repo(project.id));
	const baseRepo = $derived(baseRepoResponse.current.data);

	const settingsService = getContext(SettingsService);
	const settingsStore = settingsService.appSettings;

	let viewport: HTMLDivElement | undefined = $state();
	let contents: HTMLDivElement | undefined = $state();

	const httpsWarningBannerDismissed = projectHttpsWarningBannerDismissed(project.id);

	function shouldShowHttpsWarning() {
		if (httpsWarningBannerDismissed) return false;
		if (!baseRepo?.protocol?.startsWith('https')) return false;
		if (forge?.current.name === 'github') return false;
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

	// Redirect to workspace if we have enabled V3 feature.
	$effect(() => {
		if ($settingsStore?.featureFlags.v3) {
			goto(`/${project.id}/workspace`);
		}
	});
</script>

<div class="board-wrapper">
	<div class="scroll-viewport hide-native-scrollbar" id="board-viewport" bind:this={viewport}>
		<div class="scroll-contents" bind:this={contents}>
			<Board projectId={project.id} />
		</div>
		<Scrollbar {viewport} horz />
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

<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import WorkspaceView from '$components/WorkspaceView.svelte';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';

	const modeService = inject(MODE_SERVICE);

	const projectId = $derived(page.params.projectId!);
	const mode = $derived(modeService.mode({ projectId }));
	const uiState = inject(UI_STATE);
	const stackService = inject(STACK_SERVICE);
	const projectState = $derived(uiState.project(projectId));
	const stackId = $derived(projectState.stackId.current);

	// Check for stackId in URL query parameters
	const urlStackId = $derived(page.url.searchParams.get('stackId'));
	let scrollToStackId = $state<string | undefined>(undefined);

	const firstStackResult = $derived(stackService.stackAt(projectId, 0));
	const firstStack = $derived(firstStackResult.current.data);

	$effect(() => {
		if (urlStackId) {
			projectState.stackId.set(urlStackId);
			scrollToStackId = urlStackId;
		} else if (stackId === undefined && firstStack) {
			projectState.stackId.set(firstStack.id);
		}
	});

	function gotoEdit() {
		goto(`/${projectId}/edit`);
	}

	$effect(() => {
		if (mode.current.data?.type === 'Edit') {
			// That was causing an incorrect linting error when project.id was accessed inside the reactive block
			gotoEdit();
		}
	});

	function onScrollComplete() {
		scrollToStackId = undefined;
		const newUrl = new URL(page.url);
		newUrl.searchParams.delete('stackId');
		goto(newUrl.toString(), { replaceState: false });
	}
</script>

<WorkspaceView {projectId} {scrollToStackId} {onScrollComplete} />

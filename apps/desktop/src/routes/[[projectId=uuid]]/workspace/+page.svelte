<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import WorkspaceView from "$components/views/WorkspaceView.svelte";
	import { MODE_SERVICE } from "$lib/mode/modeService";
	import { editModePath } from "$lib/routes/routes.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import type { PageData } from "./$types";

	const { data }: { data: PageData } = $props();
	const projectId = $derived(data.projectId);

	const modeService = inject(MODE_SERVICE);
	const mode = $derived(modeService.mode(projectId));
	const stackService = inject(STACK_SERVICE);

	// Check for stackId in URL query parameters
	const urlStackId = $derived(page.url.searchParams.get("stackId"));
	let scrollToStackId = $state<string | undefined>(undefined);

	// Read all local commits in the workspace for the given project
	$effect(() => {
		stackService.allLocalCommits(projectId);
	});

	$effect(() => {
		if (urlStackId) {
			scrollToStackId = urlStackId;
		}
	});

	function gotoEdit() {
		goto(editModePath(projectId));
	}

	$effect(() => {
		if (mode.response?.type === "Edit") {
			gotoEdit();
		}
	});

	function onScrollComplete() {
		scrollToStackId = undefined;
		const newUrl = new URL(page.url);
		newUrl.searchParams.delete("stackId");
		goto(newUrl.toString(), { replaceState: false });
	}
</script>

<WorkspaceView {projectId} {scrollToStackId} {onScrollComplete} />

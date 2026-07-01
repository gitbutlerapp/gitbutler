<script lang="ts">
	import { page } from "$app/state";
	import ProjectNotFound from "$components/onboarding/ProjectNotFound.svelte";
	import RouteErrorView from "$components/shared/RouteErrorView.svelte";

	const code = $derived(page.error?.errorCode);
	const status = $derived(page.status);
	const message = $derived(page.error?.message);

	const error = $derived(message ? message : status === 404 ? "Page not found" : "Unknown error");
</script>

{#if code === "ProjectMissing"}
	<!-- We assume `projectId` is in the path given the code. -->
	{@const projectId = page.params.projectId!}
	<ProjectNotFound {projectId} />
{:else}
	<RouteErrorView {error} />
{/if}

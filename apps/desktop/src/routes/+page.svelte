<script lang="ts">
	import { goto } from "$app/navigation";
	import FullviewLoading from "$components/shared/FullviewLoading.svelte";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { inject } from "@gitbutler/core/context";
	import type { LayoutData } from "./$types";

	const { data }: { data: LayoutData } = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const pinnedId = $derived(data.pinnedProjectId);

	// When a project is pinned (remote access mode), go straight to /workspace.
	$effect(() => {
		if (pinnedId) {
			goto("/workspace", { replaceState: true });
		}
	});

	const projectsQuery = projectsService.projects();

	type Redirect =
		| {
				type: "loading" | "no-projects";
		  }
		| {
				type: "redirect";
				subject: string;
		  };

	const persistedId = projectsService.getLastOpenedProject();
	const redirect: Redirect = $derived.by(() => {
		if (pinnedId) return { type: "loading" }; // handled above
		const projects = projectsQuery.response;
		if (projects === undefined) return { type: "loading" };
		const projectId = projects.find((p) => p.id === persistedId)?.id;
		if (projectId) {
			return { type: "redirect", subject: `/${projectId}/workspace` };
		}
		if (projects.length > 0) {
			return { type: "redirect", subject: `/${projects[0]?.id}/workspace` };
		}
		return { type: "no-projects" };
	});

	$effect(() => {
		if (redirect.type === "redirect") {
			goto(redirect.subject);
		} else if (redirect.type === "no-projects") {
			goto("/onboarding");
		}
	});
</script>

{#if redirect.type === "loading"}
	<FullviewLoading />
{/if}

<script lang="ts">
	import type { Project, ProjectService } from '$lib/backend/projects';
	import IconButton from '$lib/components/IconButton.svelte';
	import type { PrService } from '$lib/github/pullrequest';
	import IconDropDown from '$lib/icons/IconDropDown.svelte';
	import ProjectsPopup from './ProjectsPopup.svelte';

	export let project: Project;
	export let projectService: ProjectService;

	$: projects$ = projectService.projects$;

	let popup: ProjectsPopup;
</script>

<div
	class="flex flex-grow items-center rounded-md p-3"
	style:background-color="var(--bg-surface-highlight)"
>
	<div class="flex flex-grow items-center gap-1 font-bold">
		{project.title}
	</div>
	<div class="flex gap-x-2">
		<IconButton
			class="items-center justify-center align-top "
			icon={IconDropDown}
			on:click={(e) => {
				popup.show();
				e.preventDefault();
			}}
		/>
	</div>
</div>
<ProjectsPopup bind:this={popup} projects={$projects$} />

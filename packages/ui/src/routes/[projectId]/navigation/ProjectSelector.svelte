<script lang="ts">
	import type { Project, ProjectService } from '$lib/backend/projects';
	import Icon from '$lib/icons/Icon.svelte';
	import { map } from 'rxjs';
	import ProjectsPopup from './ProjectsPopup.svelte';
	import { clickOutside } from './clickOutside';

	export let project: Project;
	export let projectService: ProjectService;

	$: projects$ = projectService.projects$.pipe(
		map((projects) => projects.filter((p) => p.id != project.id))
	);

	let popup: ProjectsPopup;
</script>

<div class="wrapper">
	<div
		class="relative"
		use:clickOutside={() => {
			popup.hide();
		}}
	>
		<button
			class="flex w-full items-center justify-between rounded-md p-3"
			style:background-color="var(--bg-surface-highlight)"
			on:click={(e) => {
				popup.toggle();
				e.preventDefault();
			}}
		>
			<div class="flex flex-grow items-center gap-1 font-bold">
				{project.title}
			</div>
			<Icon name="select-chevron" class="align-top" />
		</button>
		<ProjectsPopup bind:this={popup} projects={$projects$} />
	</div>
</div>

<style>
	.wrapper {
		margin-top: var(--space-10);
		margin-bottom: var(--space-16);
	}
</style>

<script lang="ts">
	import type { Project, ProjectService } from '$lib/backend/projects';
	import { clickOutside } from '$lib/clickOutside';
	import Icon from '$lib/icons/Icon.svelte';
	import ProjectsPopup from './ProjectsPopup.svelte';

	export let project: Project;
	export let projectService: ProjectService;

	let popup: ProjectsPopup;
	let visible: boolean = false;
</script>

<div class="wrapper">
	<div
		class="relative"
		use:clickOutside={{
			handler: () => {
				popup.hide();
				visible = false;
			},
			enabled: visible
		}}
	>
		<button
			class="button"
			on:click={(e) => {
				visible = popup.toggle();
				e.preventDefault();
			}}
		>
			<span class="button__label text-base-14 text-bold">{project.title}</span>
			<div class="button__icon">
				<Icon name="select-chevron" />
			</div>
		</button>
		<ProjectsPopup bind:this={popup} {projectService} />
	</div>
</div>

<style lang="postcss">
	.wrapper {
		margin-top: var(--space-10);
		margin-bottom: var(--space-16);
	}

	.button {
		display: flex;
		width: 100%;
		padding: var(--space-12);
		border-radius: var(--radius-m);

		background-color: var(--clr-theme-container-pale);

		align-items: center;
		justify-content: space-between;

		transition: background-color var(--transition-fast);

		&:focus,
		&:hover {
			background-color: var(--clr-theme-container-sub);
			& .button__icon {
				color: var(--clr-theme-scale-ntrl-50);
			}
		}
	}

	.button__label {
		flex-grow: 1;
		color: var(--clr-theme-scale-ntrl-0);
		text-align: left;
	}

	.button__icon {
		color: var(--clr-theme-scale-ntrl-60);
	}
</style>

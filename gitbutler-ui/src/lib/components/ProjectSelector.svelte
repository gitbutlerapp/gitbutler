<script lang="ts">
	import ProjectAvatar from './ProjectAvatar.svelte';
	import ProjectsPopup from './ProjectsPopup.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import Icon from '$lib/components/Icon.svelte';
	import { tooltip } from '$lib/utils/tooltip';
	import type { Project } from '$lib/backend/projects';

	export let project: Project | undefined;
	export let isNavCollapsed: boolean;

	let popup: ProjectsPopup;
	let visible: boolean = false;
</script>

<div
	class="wrapper"
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
		use:tooltip={isNavCollapsed ? project?.title : ''}
		on:mousedown={(e) => {
			visible = popup.toggle();
			e.preventDefault();
		}}
	>
		<ProjectAvatar name={project?.title} />
		{#if !isNavCollapsed}
			<span class="button__label text-base-14 text-bold">{project?.title}</span>
			<div class="button__icon">
				<Icon name="select-chevron" />
			</div>
		{/if}
	</button>
	<ProjectsPopup bind:this={popup} {isNavCollapsed} />
</div>

<style lang="postcss">
	.wrapper {
		position: relative;
		margin-top: var(--space-14);
		margin-bottom: var(--space-16);
		height: fit-content;
	}

	.button {
		display: flex;
		gap: var(--space-10);
		width: 100%;
		padding: var(--space-10);
		border-radius: var(--radius-m);

		background-color: var(--clr-theme-container-pale);

		align-items: center;
		justify-content: space-between;

		transition: background-color var(--transition-fast);

		&:focus,
		&:hover {
			background-color: color-mix(
				in srgb,
				var(--clr-theme-container-light),
				var(--clr-core-ntrl-50) 20%
			);

			& .button__icon {
				opacity: 0.4;
			}
		}
	}

	.button__label {
		flex-grow: 1;
		color: var(--clr-theme-scale-ntrl-0);
		text-align: left;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.button__icon {
		color: var(--clr-theme-scale-ntrl-0);
		opacity: 0.3;
	}
</style>

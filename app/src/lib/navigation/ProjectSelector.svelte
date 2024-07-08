<script lang="ts">
	import ProjectAvatar from './ProjectAvatar.svelte';
	import ProjectsPopup from './ProjectsPopup.svelte';
	import { Project } from '$lib/backend/projects';
	import Icon from '$lib/shared/Icon.svelte';
	import { getContext } from '$lib/utils/context';
	import { tooltip } from '$lib/utils/tooltip';

	export let isNavCollapsed: boolean;

	let buttonTrigger: HTMLButtonElement;
	const project = getContext(Project);

	let popup: ProjectsPopup;
</script>

<div class="wrapper">
	<button
		bind:this={buttonTrigger}
		class="text-input button"
		use:tooltip={isNavCollapsed ? project?.title : ''}
		on:mousedown={(e) => {
			e.preventDefault();
			popup.toggle();
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
	<ProjectsPopup bind:this={popup} target={buttonTrigger} {isNavCollapsed} />
</div>

<style lang="postcss">
	.wrapper {
		position: relative;
		margin-top: 14px;
		margin-bottom: 16px;
		height: fit-content;

		&:hover {
			& .button__icon {
				color: var(--clr-scale-ntrl-40);
			}
		}
	}

	.button {
		display: flex;
		gap: 10px;
		width: 100%;
		padding: 8px;

		align-items: center;
		justify-content: space-between;

		transition: background-color var(--transition-fast);
	}

	.button__label {
		flex-grow: 1;
		color: var(--clr-scale-ntrl-0);
		text-align: left;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.button__icon {
		display: flex;
		color: var(--clr-scale-ntrl-50);
	}
</style>

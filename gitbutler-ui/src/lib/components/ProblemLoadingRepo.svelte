<script lang="ts">
	import * as toasts from '$lib/utils/toasts';
	import type { Project, ProjectService } from '$lib/backend/projects';
	import type { UserService } from '$lib/stores/user';
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import Icon from '$lib/icons/Icon.svelte';
	import IconLink from './IconLink.svelte';
	import Button from './Button.svelte';
	import ProjectSwitcher from './ProjectSwitcher.svelte';
	import { goto } from '$app/navigation';
	import Modal from './Modal.svelte';

	export let projectService: ProjectService;
	export let project: Project;
	export let userService: UserService;
	export let error: any = undefined;

	$: user$ = userService.user$;

	let loading = false;
	let deleteConfirmationModal: Modal;

	async function onDeleteClicked() {
		loading = true;
		try {
			deleteConfirmationModal.close();
			await projectService.deleteProject(project.id);
			toasts.success('Project deleted');
			goto('/');
		} catch (e) {
			console.error(e);
			toasts.error('Failed to delete project');
		} finally {
			loading = false;
			projectService.reload();
		}
	}
</script>

<DecorativeSplitView
	user={$user$}
	imgSet={{
		light: '/images/img_repo-load-error-light.webp',
		dark: '/images/img_repo-load-error-dark.webp'
	}}
>
	<div class="problem" data-tauri-drag-region>
		<p class="problem__project text-bold"><Icon name="repo-book" /> {project?.title}</p>
		<p class="problem__title text-base-body-18 text-bold" data-tauri-drag-region>
			There was a problem loading this repo
		</p>

		{#if error}
			<div class="problem__error text-base-body-12">
				<Icon name="error" color="error" />
				{error}
			</div>
		{/if}

		<div class="problem__switcher">
			<ProjectSwitcher {projectService} {project} />
		</div>

		<div class="problem__delete">
			<Button
				wide
				on:click={() => deleteConfirmationModal.show()}
				{loading}
				kind="outlined"
				color="error"
				icon="bin-small"
			>
				Remove project from GitButler
			</Button>
		</div>
	</div>
	<svelte:fragment slot="links">
		<IconLink icon="docs" href="https://docs.gitbutler.com/features/virtual-branches/branch-lanes">
			GitButler Docs
		</IconLink>
		<IconLink icon="video" href="https://www.youtube.com/@gitbutlerapp">Watch tutorial</IconLink>
	</svelte:fragment>
</DecorativeSplitView>

<Modal bind:this={deleteConfirmationModal} title="Delete {project.title}?">
	<p>
		Are you sure you want to delete
		<span class="font-bold">{project.title}</span>? This can’t be undone.
	</p>

	<svelte:fragment slot="controls" let:close>
		<Button kind="outlined" on:click={close}>Cancel</Button>
		<Button color="error" {loading} on:click={onDeleteClicked}>Delete project</Button>
	</svelte:fragment>
</Modal>

<style lang="postcss">
	.problem__project {
		display: flex;
		gap: var(--space-8);
		align-items: center;
		line-height: 120%;
		color: var(--clr-theme-scale-ntrl-30);
		margin-bottom: var(--space-20);
	}

	.problem__title {
		color: var(--clr-theme-scale-ntrl-30);
		margin-bottom: var(--space-12);
	}

	.problem__switcher {
		text-align: right;
		margin-top: var(--space-10);
		padding-bottom: var(--space-32);
		border-bottom: 1px dashed var(--clr-theme-scale-ntrl-60);
	}

	.problem__error {
		display: flex;
		color: var(--clr-theme-scale-ntrl-0);
		gap: var(--space-12);
		padding: var(--space-20);
		background-color: var(--clr-theme-err-container);
		border-radius: var(--radius-m);
		margin-bottom: var(--space-24);
	}

	.problem__delete {
		margin-top: var(--space-32);
	}
</style>

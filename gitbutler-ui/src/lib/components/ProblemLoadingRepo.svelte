<script lang="ts">
	import type { Project, ProjectService } from '$lib/backend/projects';
	import type { UserService } from '$lib/stores/user';
	import ProjectSelector from '../../routes/[projectId]/navigation/ProjectSelector.svelte';
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import IconLink from './IconLink.svelte';

	export let projectService: ProjectService;
	export let project: Project | undefined;
	export let userService: UserService;

	$: user$ = userService.user$;

	const SLOTS = $$props.$$slots;
</script>

<DecorativeSplitView
	user={$user$}
	imgSet={{
		light: '/images/img_hmm-path-light.webp',
		dark: '/images/img_hmm-path-dark.webp'
	}}
>
	<div class="welcome">
		<p class="title text-base-body-18 text-bold">
			{#if SLOTS.title}
				<slot name="title" />
			{:else}
				There was a problem loading this repo
			{/if}
		</p>
		<p class="message">
			<slot />
		</p>
		{#if SLOTS.actions}
			<div class="actions">
				<slot name="actions" />
			</div>
		{/if}

		<ProjectSelector {projectService} {project}></ProjectSelector>
	</div>
	<svelte:fragment slot="links">
		<IconLink icon="docs" href="https://docs.gitbutler.com/features/virtual-branches/branch-lanes">
			GitButler Docs
		</IconLink>
		<IconLink icon="video" href="https://www.youtube.com/@gitbutlerapp">Watch tutorial</IconLink>
	</svelte:fragment>
</DecorativeSplitView>

<style lang="postcss">
	.welcome {
		width: 27.25rem;
	}

	.title {
		color: var(--clr-theme-scale-ntrl-30);
		margin-bottom: var(--space-12);
	}

	.message {
		color: var(--clr-theme-scale-ntrl-50);
		margin-bottom: var(--space-24);
	}

	.actions {
		margin-bottom: var(--space-24);
	}
</style>

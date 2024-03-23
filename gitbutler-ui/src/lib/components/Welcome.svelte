<script lang="ts">
	import WelcomeAction from './WelcomeAction.svelte';
	import WelcomeSigninAction from './WelcomeSigninAction.svelte';
	import newProjectSvg from '$lib/assets/no-projects/new-project.svg?raw';
	import { ProjectService } from '$lib/backend/projects';
	import IconLink from '$lib/components/IconLink.svelte';
	import { getContextByClass } from '$lib/utils/context';

	const projectService = getContextByClass(ProjectService);

	let newProjectLoading = false;

	async function onNewProject() {
		newProjectLoading = true;
		try {
			await projectService.addProject();
		} finally {
			newProjectLoading = false;
		}
	}
</script>

<div class="welcome">
	<h1 class="welcome-title text-serif-40">Welcome to GitButler</h1>
	<div class="welcome__actions">
		<WelcomeAction title="Add new project" loading={newProjectLoading} on:mousedown={onNewProject}>
			<svelte:fragment slot="icon">
				{@html newProjectSvg}
			</svelte:fragment>
			<svelte:fragment slot="message">
				Verify valid Git repository in selected folder before importing.
			</svelte:fragment>
		</WelcomeAction>
		<!-- Using instance of user here to not hide after login -->
		<WelcomeSigninAction />
	</div>

	<div class="links">
		<div class="links__section">
			<p class="links__title text-base-14 text-bold">Quick start</p>
			<div class="education-links">
				<IconLink
					icon="docs"
					href="https://docs.gitbutler.com/features/virtual-branches/branch-lanes"
				>
					GitButler docs
				</IconLink>
				<IconLink icon="video" href="https://www.youtube.com/@gitbutlerapp">
					Watch tutorials
				</IconLink>
			</div>
		</div>
		<div class="links__section">
			<p class="links__title text-base-14 text-bold">Join our community</p>
			<div class="community-links">
				<IconLink icon="discord" href="https://discord.gg/MmFkmaJ42D">Discord</IconLink>
				<IconLink icon="x" href="https://twitter.com/gitbutler">X</IconLink>
				<IconLink icon="instagram" href="https://www.instagram.com/gitbutler/">Instagram</IconLink>
			</div>
		</div>
	</div>
</div>

<style lang="postcss">
	.welcome {
		width: 100%;
	}

	.welcome-title {
		color: var(--clr-theme-scale-ntrl-0);
		line-height: 1;
	}

	.welcome__actions {
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
		margin-top: var(--size-32);
	}

	.links {
		display: flex;
		gap: var(--size-56);
		padding: var(--size-28);
		background: var(--clr-theme-container-pale);
		border-radius: var(--radius-m);
		margin-top: var(--size-20);
	}

	.links__section {
		display: flex;
		flex-direction: column;
		gap: var(--size-20);
	}

	.education-links {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--size-6);
		margin-left: calc(var(--size-6) * -1);
	}

	.community-links {
		display: flex;
		flex-wrap: wrap;
		column-gap: var(--size-12);
		row-gap: var(--size-4);
		max-width: calc(var(--size-64) * 3);
		margin-left: calc(var(--size-6) * -1);
	}
</style>

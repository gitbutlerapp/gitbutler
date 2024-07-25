<script lang="ts">
	import WelcomeAction from './WelcomeAction.svelte';
	import WelcomeSigninAction from './WelcomeSigninAction.svelte';
	import newProjectSvg from '$lib/assets/no-projects/new-project.svg?raw';
	import { ProjectService } from '$lib/backend/projects';
	import IconLink from '$lib/shared/IconLink.svelte';
	import { getContext } from '$lib/utils/context';

	const projectService = getContext(ProjectService);

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
		color: var(--clr-scale-ntrl-0);
		line-height: 1;
	}

	.welcome__actions {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin-top: 32px;
	}

	.links {
		display: flex;
		gap: 56px;
		padding: 28px;
		background: var(--clr-bg-2);
		border-radius: var(--radius-m);
		margin-top: 20px;
	}

	.links__section {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.education-links {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 6px;
		margin-left: -6px;
	}

	.community-links {
		display: flex;
		flex-wrap: wrap;
		column-gap: 12px;
		row-gap: 4px;
		max-width: 192px;
		margin-left: -6px;
	}
</style>

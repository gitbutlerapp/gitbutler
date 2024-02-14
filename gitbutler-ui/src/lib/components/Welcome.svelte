<script lang="ts">
	import WelcomeAction from './WelcomeAction.svelte';
	import IconLink from '$lib/components/IconLink.svelte';
	import ImgThemed from '$lib/components/ImgThemed.svelte';
	import type { ProjectService } from '$lib/backend/projects';
	import type { UserService } from '$lib/stores/user';

	export let projectService: ProjectService;
	export let userService: UserService;

	let newProjectLoading = false;
	let loginSignupLoading = false;

	const user$ = userService.user$;

	async function onNewProject() {
		newProjectLoading = true;
		try {
			await projectService.addProject();
		} finally {
			newProjectLoading = false;
		}
	}

	async function onLoginOrSignup() {
		loginSignupLoading = true;
		try {
			await userService.login();
		} catch {
			loginSignupLoading = false;
		}
	}

	// reset loading state after 60 seconds
	// this is to prevent the loading state from getting stuck
	// if the user closes the tab before the request is finished
	setTimeout(() => {
		loginSignupLoading = false;
	}, 60 * 1000);
</script>

<div class="welcome">
	<h1 class="welcome-title text-serif-40">Welcome to GitButler</h1>
	<div class="welcome__actions">
		<WelcomeAction title="Add new project" loading={newProjectLoading} on:click={onNewProject}>
			<svelte:fragment slot="icon">
				<ImgThemed
					imgSet={{
						light: '/images/welcome-new-project-light.webp',
						dark: '/images/welcome-new-project-dark.webp'
					}}
				/>
			</svelte:fragment>
			<svelte:fragment slot="message">
				Verify valid Git repository in selected folder before importing.
			</svelte:fragment>
		</WelcomeAction>
		<!-- Using instance of user here to not hide after login -->
		{#if !$user$}
			<WelcomeAction
				title="Log in or Sign up"
				loading={loginSignupLoading}
				on:click={onLoginOrSignup}
			>
				<svelte:fragment slot="icon">
					<ImgThemed
						imgSet={{
							light: '/images/welcome-signin-light.webp',
							dark: '/images/welcome-signin-dark.webp'
						}}
					/>
				</svelte:fragment>
				<svelte:fragment slot="message">
					Enable GitButler features like automatic branch and commit message generation.
				</svelte:fragment>
			</WelcomeAction>
		{/if}
	</div>

	<div class="links">
		<div class="links__section">
			<p class="links__title text-base-14 text-bold">Quick start</p>
			<div class="education-links">
				<IconLink
					icon="docs"
					href="https://docs.gitbutler.com/features/virtual-branches/branch-lanes"
				>
					GitButler Docs
				</IconLink>
				<IconLink icon="video" href="https://www.youtube.com/@gitbutlerapp">
					Watch tutorials
				</IconLink>
			</div>
		</div>
		<div class="links__section">
			<p class="links__title text-base-14 text-bold">Join our community</p>
			<div class="community-links">
				<IconLink icon="discord" href="https://discord.gg/wDKZCPEjXC">Discord</IconLink>
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
		gap: var(--space-8);
		margin-top: var(--space-32);
	}

	.links {
		display: flex;
		gap: var(--space-56);
		padding: var(--space-28);
		background: var(--clr-theme-container-pale);
		border-radius: var(--radius-l);
		margin-top: var(--space-20);
	}

	.links__section {
		display: flex;
		flex-direction: column;
		gap: var(--space-20);
	}

	.education-links {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-6);
		margin-left: calc(var(--space-6) * -1);
	}

	.community-links {
		display: flex;
		flex-wrap: wrap;
		column-gap: var(--space-12);
		row-gap: var(--space-4);
		max-width: calc(var(--space-64) * 3);
		margin-left: calc(var(--space-6) * -1);
	}
</style>

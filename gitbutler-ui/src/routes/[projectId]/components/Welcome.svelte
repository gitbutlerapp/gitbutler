<script lang="ts">
	import type { ProjectService } from '$lib/backend/projects';
	import IconLink from '$lib/components/IconLink.svelte';
	import type { UserService } from '$lib/stores/user';
	import WelcomeAction from './WelcomeAction.svelte';

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
</script>

<div class="welcome">
	<p class="text-serif-40">Welcome to GitButler</p>
	<div class="welcome__actions">
		<WelcomeAction title="Add new project" loading={newProjectLoading} on:click={onNewProject}>
			<svelte:fragment slot="icon">
				<!-- prettier-ignore -->
				<svg width="80" height="80" viewBox="0 0 80 80" fill="none" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
                <path d="M12 62C12 56.4772 16.4772 52 22 52H68V72H22C16.4772 72 12 67.5228 12 62Z" fill="#C1F0ED"/>
                <g style="mix-blend-mode:overlay">
                <path d="M12 62C12 56.4772 16.4772 52 22 52H68V72H22C16.4772 72 12 67.5228 12 62Z" fill="url(#pattern0)" fill-opacity="0.2"/>
                </g>
                <path d="M12 16C12 10.4772 16.4772 6 22 6H68V52H22C16.4772 52 12 56.4772 12 62V63V16Z" fill="#AFEAE7"/>
                <path d="M12 16C12 10.4772 16.4772 6 22 6H68V52H22C16.4772 52 12 56.4772 12 62V63V16Z" fill="url(#pattern1)" fill-opacity="0.6"/>
                <path d="M12 16C12 10.4772 16.4772 6 22 6H68V52H22C16.4772 52 12 56.4772 12 62V63V16Z" fill="url(#pattern2)" fill-opacity="0.7"/>
                <path d="M24 61H42V80L33 72L24 80V61Z" fill="#5DC2BD"/>
                <path d="M41 18V42M53 30L29 30" stroke="#27A7A1" stroke-width="2"/>
                <defs>
                <pattern id="pattern0" patternContentUnits="objectBoundingBox" width="0.571429" height="1.6">
                <use xlink:href="#image0_454_15688" transform="scale(0.00892857 0.025)"/>
                </pattern>
                <pattern id="pattern1" patternContentUnits="objectBoundingBox" width="1" height="1">
                <use xlink:href="#image1_454_15688" transform="matrix(0.0165604 0 0 0.0162699 0.488318 0.372868)"/>
                </pattern>
                <pattern id="pattern2" patternContentUnits="objectBoundingBox" width="1" height="1">
                <use xlink:href="#image1_454_15688" transform="matrix(0.0165604 0 0 0.0162699 -0.857117 0.673284)"/>
                </pattern>
                <image id="image0_454_15688" width="64" height="64" xlink:href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAAAXNSR0IArs4c6QAAAONJREFUeF7t20EOhEAIRFG4/6F7DvEnYeFzryQIv6pBd2behOu9dPvsbog+k+NLgArQAqmJcw9iAAhSgZKB3IJkkAySQTJ4CiE+gA8oBeg0mH3Ai084P89HhqwEqIA209ICsQdjAeaZIgaAYKxBDMCAYy8fXwAIgiAIcoJpJEYGI4VjB3YrbC9gL2AvkCB43cM5PgZgAAZgQFnNZAhdGykQBEEQBEEQDBmgAm2glM/z+QUYisYUGoldO7kY32IEAzCg6RgIRgjFAsw+AgRBMNYgBmCAT2TCYfoPPz/HCqQCX1eBHzHnv7C7WhBSAAAAAElFTkSuQmCC"/>
                <image id="image1_454_15688" width="99" height="99" xlink:href="data:image/png;base64,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"/>
                </defs>
            </svg>
			</svelte:fragment>
			<svelte:fragment slot="message">
				The project should be a valid git repo. Read more here.
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
					<!-- prettier-ignore -->
					<svg width="80" height="80" viewBox="0 0 80 80" fill="none" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
                <path d="M59.8764 41.5293L49.8828 41.5293L49.8828 60.6079H59.8764L59.8764 41.5293Z" fill="#ACE2DF"/>
                <path d="M71.687 41.5293L61.6934 41.5293L61.6934 60.6079H71.687V41.5293Z" fill="#ACE2DF"/>
                <path d="M71.687 26.9937L36.2552 26.9937L36.2552 46.0723L71.687 46.0723V26.9937Z" fill="#9EDCD9"/>
                <path d="M60.7848 26.9937H55.3337V32.4447H60.7848V37.8957H58.0592V39.7127H60.7848V37.8957H66.2358V43.3468H55.3337V37.8957H49.8827V32.4447H44.4316V37.8957H49.8827V43.3468H44.4316V46.0723H49.8827V43.3468H55.3337V46.0723H66.2358V43.3468H71.6868V37.8957H66.2358V32.4447H71.6868V26.9937H66.2358V32.4447H60.7848V26.9937Z" fill="#67B2AE"/>
                <path d="M44.4316 26.9937H49.8827V28.8107H44.4316V26.9937Z" fill="#67B2AE"/>
                <path fill-rule="evenodd" clip-rule="evenodd" d="M9 35.6244C9 25.3384 17.3384 17 27.6244 17C37.9103 17 46.2488 25.3384 46.2488 35.6244V36.5329C46.2488 46.8189 37.9103 55.1573 27.6244 55.1573C17.3384 55.1573 9 46.8189 9 36.5329L9 35.6244ZM34.6913 36.2428C34.6913 32.2779 31.4771 29.0637 27.5122 29.0637C23.5473 29.0637 20.3331 32.2779 20.3331 36.2428C20.3331 40.2077 23.5473 43.4218 27.5122 43.4218C31.4771 43.4218 34.6913 40.2077 34.6913 36.2428Z" fill="#A2E1DE"/>
                <path fill-rule="evenodd" clip-rule="evenodd" d="M9 35.6244C9 25.3384 17.3384 17 27.6244 17C37.9103 17 46.2488 25.3384 46.2488 35.6244V36.5329C46.2488 46.8189 37.9103 55.1573 27.6244 55.1573C17.3384 55.1573 9 46.8189 9 36.5329L9 35.6244ZM34.6913 36.2428C34.6913 32.2779 31.4771 29.0637 27.5122 29.0637C23.5473 29.0637 20.3331 32.2779 20.3331 36.2428C20.3331 40.2077 23.5473 43.4218 27.5122 43.4218C31.4771 43.4218 34.6913 40.2077 34.6913 36.2428Z" fill="url(#pattern0)" fill-opacity="0.7"/>
                <defs>
                <pattern id="pattern0" patternContentUnits="objectBoundingBox" width="1" height="1">
                <use xlink:href="#image0_454_15699" transform="matrix(0.0237451 0 0 0.0233285 -1.39549 0.0238096)"/>
                </pattern>
                <image id="image0_454_15699" width="99" height="99" xlink:href="data:image/png;base64,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"/>
                </defs>
            </svg>
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
			<div class="links__collection">
				<IconLink
					icon="docs"
					href="https://docs.gitbutler.com/features/virtual-branches/branch-lanes"
				>
					GitButler Docs
				</IconLink>
				<IconLink icon="video" href="https://www.youtube.com/@gitbutlerapp">
					Watch tutorial
				</IconLink>
			</div>
		</div>
		<div class="links__section">
			<p class="links__title text-base-14 text-bold">Join our community</p>
			<div class="links__collection">
				<IconLink icon="discord" href="https://discord.gg/wDKZCPEjXC">Discord</IconLink>
				<IconLink icon="instagram" href="https://www.instagram.com/gitbutler/">Instagram</IconLink>
				<IconLink icon="x" href="https://twitter.com/gitbutler">X</IconLink>
			</div>
		</div>
	</div>
</div>

<style lang="postcss">
	.welcome {
		width: 27.25rem;
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
		border-radius: var(--radius-m);
		margin-top: var(--space-20);
	}

	.links__section {
		display: flex;
		flex-direction: column;
		gap: var(--space-20);
	}
	.links__collection {
		display: flex;
		flex-direction: column;
		gap: var(--space-12);
	}
</style>

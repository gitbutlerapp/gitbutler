<script lang="ts">
	import BaseBranchCard from './BaseBranchCard.svelte';
	import Branches from './Branches.svelte';
	import DomainButton from './DomainButton.svelte';
	import Footer from './Footer.svelte';
	import ProjectSelector from './ProjectSelector.svelte';
	import Resizer from './Resizer.svelte';
	import { navCollapsed } from '$lib/config/config';
	import { persisted } from '$lib/persisted/persisted';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { type Platform, platform } from '@tauri-apps/api/os';
	import { getContext } from 'svelte';
	import type { User } from '$lib/backend/cloud';
	import type { Project, ProjectService } from '$lib/backend/projects';
	import type { BranchService } from '$lib/branches/service';
	import type { GitHubService } from '$lib/github/service';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranchService } from '$lib/vbranches/branchStoresCache';

	export let branchService: BranchService;
	export let baseBranchService: BaseBranchService;
	export let branchController: BranchController;
	export let project: Project;
	export let user: User | undefined;
	export let githubService: GitHubService;
	export let projectService: ProjectService;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
	const defaultTrayWidthRem = persisted<number | undefined>(
		undefined,
		'defaulTrayWidth_ ' + project.id
	);

	let viewport: HTMLDivElement;

	function toggleNavCollapse() {
		$isNavCollapsedPersist = !$isNavCollapsedPersist;
		isNavCollapsed = $isNavCollapsedPersist;
	}

	$: isNavCollapsedPersist = navCollapsed();
	let isNavCollapsed = $isNavCollapsedPersist;

	// Detect is the platform
	let platformName: Platform | undefined;

	platform().then((name) => {
		platformName = name;
		console.log('platformName:', platformName);
	});

	// check if resizing
	let isResizerDragging = false;
	// current resizer width
	const minResizerWidth = 280;
	const minResizerRatio = 150;
</script>

<aside class="navigation-wrapper">
	<div class="resizer-wrapper" class:resizerDragging={isResizerDragging} tabindex="0" role="button">
		<button
			class="folding-button"
			on:click={toggleNavCollapse}
			class:folding-button_folded={isNavCollapsed}
		>
			<svg
				xmlns="http://www.w3.org/2000/svg"
				xmlns:xlink="http://www.w3.org/1999/xlink"
				viewBox="0 0 8 12"
				fill="none"
				><path
					d="M6,0L0,6l6,6"
					transform="translate(1 0)"
					stroke-width="1.5"
					stroke-linejoin="round"
				/></svg
			>
		</button>
		<Resizer
			{viewport}
			direction="right"
			minWidth={minResizerWidth}
			defaultLineColor="var(--clr-theme-container-outline-light)"
			on:click={() => {
				if ($isNavCollapsedPersist) {
					toggleNavCollapse();
				}
			}}
			on:dblclick={() => {
				if (!$isNavCollapsedPersist) {
					toggleNavCollapse();
				}
			}}
			on:width={(e) => {
				$defaultTrayWidthRem = e.detail / (16 * $userSettings.zoom);
			}}
			on:resizing={(e) => {
				isResizerDragging = e.detail;
			}}
			on:overflowValue={(e) => {
				const overflowValue = e.detail;

				if (!$isNavCollapsedPersist && overflowValue > minResizerRatio) {
					$isNavCollapsedPersist = true;
					isNavCollapsed = $isNavCollapsedPersist;
				}

				if ($isNavCollapsedPersist && overflowValue < minResizerRatio) {
					$isNavCollapsedPersist = false;
					isNavCollapsed = $isNavCollapsedPersist;
				}
			}}
		/>
	</div>

	<div
		class="navigation"
		class:collapsed={isNavCollapsed}
		style:width={$defaultTrayWidthRem && !isNavCollapsed ? $defaultTrayWidthRem + 'rem' : null}
		bind:this={viewport}
		role="menu"
		tabindex="0"
	>
		{#if platformName}
			<div class="navigation-top">
				{#if platformName === 'darwin'}
					<div class="drag-region" data-tauri-drag-region />
				{/if}
				<ProjectSelector {project} {projectService} {isNavCollapsed} />
				<div class="domains">
					<BaseBranchCard {project} {baseBranchService} {githubService} {isNavCollapsed} />
					<DomainButton
						href={`/${project.id}/board`}
						domain="workspace"
						label="Workspace"
						iconSrc="/images/domain-icons/working-branches.svg"
						{branchController}
						{baseBranchService}
						{isNavCollapsed}
					></DomainButton>
				</div>
			</div>
			{#if !isNavCollapsed}
				<Branches projectId={project.id} {branchService} {githubService} />
			{/if}
			<Footer {user} projectId={project.id} {isNavCollapsed} />
		{/if}
	</div>
</aside>

<style lang="postcss">
	.navigation-wrapper {
		display: flex;
		position: relative;

		&:hover {
			& .folding-button {
				opacity: 1;
				transform: translateY(-50%);
				right: calc(var(--space-6) * -1);
				transition-delay: 0.1s;

				& svg {
					transition-delay: 0.1s;
				}
			}
		}
	}
	.navigation {
		width: 17.5rem;
		display: flex;
		flex-direction: column;
		position: relative;
		background: var(--clr-theme-container-light);
		max-height: 100%;
		user-select: none;
	}
	.drag-region {
		flex-shrink: 0;
		height: var(--space-24);
	}
	.navigation-top {
		display: flex;
		flex-direction: column;
		padding-bottom: var(--space-24);
		padding-left: var(--space-14);
		padding-right: var(--space-14);
	}
	.domains {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	.resizer-wrapper {
		position: absolute;
		top: 0;
		right: 0;
		height: 100%;
		width: var(--space-4);

		&:hover,
		&.resizerDragging {
			& .folding-button {
				background-color: var(--resizer-color);
				border: 1px solid var(--resizer-color);

				& svg {
					stroke: var(--clr-theme-scale-ntrl-100);
				}
			}
		}
	}

	/* FOLDING BUTTON */

	.folding-button {
		z-index: 42;
		position: absolute;
		right: calc(var(--space-2) * -1);
		top: 50%;
		transform: translateY(-50%);
		width: var(--space-16);
		height: var(--space-36);
		padding: var(--space-4);
		background: var(--clr-theme-container-light);
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-theme-container-outline-light);
		opacity: 0;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast),
			opacity var(--transition-medium),
			all var(--transition-medium);

		& svg {
			stroke: var(--clr-theme-scale-ntrl-50);
			transition: stroke var(--transition-fast);
		}
	}

	.folding-button_folded {
		& svg {
			transform: rotate(180deg) translateX(-0.0625rem);
		}
	}

	/* COLLAPSED */
	.navigation.collapsed {
		width: auto;
		justify-content: space-between;
		padding-bottom: var(--space-16);
	}
</style>

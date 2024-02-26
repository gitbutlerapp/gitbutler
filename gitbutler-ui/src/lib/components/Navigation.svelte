<script lang="ts">
	import Branches from './Branches.svelte';
	import DomainButton from './DomainButton.svelte';
	import Footer from './Footer.svelte';
	import ProjectSelector from './ProjectSelector.svelte';
	import UpdateBaseButton from './UpdateBaseButton.svelte';
	import BaseBranchCard from './BaseBranchCard.svelte';
	import Resizer from './Resizer.svelte';
	import Button from './Button.svelte';
	import { navCollapsed } from '$lib/config/config';
	import { persisted, type Persisted } from '$lib/persisted/persisted';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
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
</script>

{#if isNavCollapsed}
	<div class="collapsed-nav-wrapper">
		<div class="card collapsed-nav">
			<Button
				icon="unfold-lane"
				kind="outlined"
				color="neutral"
				help="Collapse Nav"
				on:click={toggleNavCollapse}
			/>
			<div class="collapsed-nav__info">
				<h3 class="collapsed-nav__label text-base-13 text-bold">
					{project?.title}
				</h3>
				<DomainButton
					href={`/${project.id}/board`}
					domain="workspace"
					{branchController}
					{baseBranchService}
					{isNavCollapsed}
				></DomainButton>
				<BaseBranchCard {project} {baseBranchService} {githubService} {isNavCollapsed} />
			</div>
			<div class="collapsed-nav__footer">
				<Footer {user} projectId={project.id} {isNavCollapsed} />
			</div>
		</div>
	</div>
{:else}
	<div
		class="navigation relative flex w-80 shrink-0 flex-col border-r"
		style:width={$defaultTrayWidthRem ? $defaultTrayWidthRem + 'rem' : null}
		bind:this={viewport}
		role="menu"
		tabindex="0"
	>
		<div class="drag-region" data-tauri-drag-region></div>
		<div class="hide-nav-button">
			<Button
				icon="fold-lane"
				kind="outlined"
				color="neutral"
				help="Collapse Nav"
				align="flex-end"
				on:click={toggleNavCollapse}
			/>
		</div>
		<div class="domains">
			<ProjectSelector {project} {projectService} />
			<div class="flex flex-col gap-1">
				<BaseBranchCard {project} {baseBranchService} {githubService} {isNavCollapsed} />
				<DomainButton
					href={`/${project.id}/board`}
					domain="workspace"
					{branchController}
					{baseBranchService}
					{isNavCollapsed}
				></DomainButton>
			</div>
		</div>
		<Branches projectId={project.id} {branchService} {githubService} />
		<Footer {user} projectId={project.id} {isNavCollapsed} />

		<Resizer
			{viewport}
			direction="right"
			minWidth={320}
			on:width={(e) => {
				$defaultTrayWidthRem = e.detail / (16 * $userSettings.zoom);
			}}
		/>
	</div>
{/if}

<style lang="postcss">
	.navigation {
		border-right: 1px solid var(--clr-theme-container-outline-light);
		background: var(--clr-theme-container-light);
		max-height: 100%;
		user-select: none;
	}
	.drag-region {
		flex-shrink: 0;
		height: var(--space-24);
	}
	.domains {
		padding-bottom: var(--space-24);
		padding-left: var(--space-12);
		padding-right: var(--space-12);
	}
	.hide-nav-button {
		align-self: flex-end;
		margin-right: var(--space-12);
	}
	.collapsed-nav-wrapper {
		padding: var(--space-12);
		height: 100%;
		border-right: 1px solid var(--clr-theme-container-outline-light);
	}
	.collapsed-nav {
		display: flex;
		flex-direction: column;
		cursor: default;
		user-select: none;
		height: 100%;
		gap: var(--space-8);
		padding: var(--space-8) var(--space-8) var(--space-20);

		&:focus-within {
			outline: none;
		}
	}
	.collapsed-nav__info {
		flex: 1;
		display: flex;
		flex-direction: row-reverse;
		align-items: center;
		justify-content: flex-end;
		height: 100%;

		writing-mode: vertical-rl;
		gap: var(--space-8);
	}
	.collapsed-nav__label {
		color: var(--clr-theme-scale-ntrl-0);
		transform: rotate(180deg);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		padding-bottom: var(--space-8);
	}
	.collapsed-nav__footer {
		align-self: flex-end;

		writing-mode: vertical-rl;
		gap: var(--space-8);
	}
</style>

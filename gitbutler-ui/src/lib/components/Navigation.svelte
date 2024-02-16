<script lang="ts">
	import Branches from './Branches.svelte';
	import DomainButton from './DomainButton.svelte';
	import Footer from './Footer.svelte';
	import ProjectSelector from './ProjectSelector.svelte';
	import UpdateBaseButton from './UpdateBaseButton.svelte';
	import BaseBranchCard from '$lib/components/BaseBranchCard.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import { persisted } from '$lib/persisted/persisted';
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

	$: base$ = baseBranchService.base$;

	let viewport: HTMLDivElement;
</script>

<div
	class="navigation relative flex w-80 shrink-0 flex-col border-r"
	style:width={$defaultTrayWidthRem ? $defaultTrayWidthRem + 'rem' : null}
	bind:this={viewport}
	role="menu"
	tabindex="0"
>
	<div class="drag-region" data-tauri-drag-region></div>
	<div class="domains">
		<ProjectSelector {project} {projectService} />
		<div class="flex flex-col gap-1">
			<BaseBranchCard {project} {baseBranchService} {githubService} />
			<DomainButton href={`/${project.id}/board`}>
				<svg
					width="16"
					height="16"
					viewBox="0 0 16 16"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
				>
					<path
						d="M0 6.64C0 4.17295 0 2.93942 0.525474 2.01817C0.880399 1.39592 1.39592 0.880399 2.01817 0.525474C2.93942 0 4.17295 0 6.64 0H9.36C11.8271 0 13.0606 0 13.9818 0.525474C14.6041 0.880399 15.1196 1.39592 15.4745 2.01817C16 2.93942 16 4.17295 16 6.64V9.36C16 11.8271 16 13.0606 15.4745 13.9818C15.1196 14.6041 14.6041 15.1196 13.9818 15.4745C13.0606 16 11.8271 16 9.36 16H6.64C4.17295 16 2.93942 16 2.01817 15.4745C1.39592 15.1196 0.880399 14.6041 0.525474 13.9818C0 13.0606 0 11.8271 0 9.36V6.64Z"
						fill="#48B0AA"
					/>
					<rect x="2" y="3" width="6" height="10" rx="2" fill="#D9F3F2" />
					<rect opacity="0.7" x="10" y="3" width="4" height="10" rx="2" fill="#D9F3F2" />
				</svg>

				<span>Workspace</span>
				{#if ($base$?.behind || 0) > 0}
					<UpdateBaseButton {branchController} />
				{/if}
			</DomainButton>
		</div>
	</div>
	<Branches projectId={project.id} {branchService} {githubService} />
	<Footer {user} projectId={project.id} />

	<Resizer
		{viewport}
		direction="right"
		minWidth={320}
		on:width={(e) => {
			$defaultTrayWidthRem = e.detail / (16 * $userSettings.zoom);
		}}
	/>
</div>

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
</style>

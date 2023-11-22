<script lang="ts">
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { getContext } from 'svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { User } from '$lib/backend/cloud';
	import BaseBranchCard from './BaseBranchCard.svelte';
	import type { Project, ProjectService } from '$lib/backend/projects';
	import StashedBranches from './StashedBranches.svelte';
	import Footer from './Footer.svelte';
	import AppUpdater from './AppUpdater.svelte';
	import type { Loadable } from '@square/svelte-store';
	import type { Update } from '../../updater';
	import DomainButton from './DomainButton.svelte';
	import type { PrService } from '$lib/github/pullrequest';
	import type { BaseBranchService, VirtualBranchService } from '$lib/vbranches/branchStoresCache';
	import ProjectSelector from './ProjectSelector.svelte';
	import Branches from './Branches.svelte';
	import type { BranchService } from '$lib/branches/service';
	import Header from './Header.svelte';

	export let vbranchService: VirtualBranchService;
	export let branchService: BranchService;
	export let baseBranchService: BaseBranchService;
	export let branchController: BranchController;
	export let project: Project;
	export let user: User | undefined;
	export let update: Loadable<Update>;
	export let prService: PrService;
	export let projectService: ProjectService;

	$: activeBranches$ = vbranchService?.activeBranches$;
	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	let stashExpanded = true;
</script>

<div
	class="z-30 flex w-80 shrink-0 flex-col border-r"
	style:background-color="var(--bg-surface)"
	style:border-color="var(--border-surface)"
	style:width={$userSettings.trayWidth ? `${$userSettings.trayWidth}px` : null}
	role="menu"
	tabindex="0"
>
	<div class="domains">
		<Header {branchController} {baseBranchService} {prService} />
		<ProjectSelector {project} {projectService} />
		<div class="flex flex-col gap-1">
			<BaseBranchCard {project} {baseBranchService} {branchController} />
			<DomainButton href={`/${project.id}/board`}>
				<svg
					width="16"
					height="16"
					viewBox="0 0 16 16"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
				>
					<rect width="16" height="16" rx="4" fill="#797FE6" />
					<path d="M5 8.8H11V4" stroke="white" stroke-width="2" />
					<path d="M5 12V8.44444V4" stroke="white" stroke-width="2" />
				</svg>

				Applied branches
			</DomainButton>
		</div>
	</div>
	<Branches
		projectId={project.id}
		{branchService}
		grow={!stashExpanded || $activeBranches$?.length == 0}
	/>
	<StashedBranches {project} {branchController} {vbranchService} bind:expanded={stashExpanded} />
	<Footer {user} projectId={project.id} />
	<AppUpdater {update} />
</div>

<style lang="postcss">
	.domains {
		padding-top: var(--space-14);
		padding-bottom: var(--space-24);
		padding-left: var(--space-16);
		padding-right: var(--space-16);
	}
</style>

<script lang="ts">
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { getContext } from 'svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { User } from '$lib/backend/cloud';
	import BaseBranchCard from './BaseBranchCard.svelte';
	import type { Project, ProjectService } from '$lib/backend/projects';
	import YourBranches from './YourBranches.svelte';
	import Footer from './Footer.svelte';
	import AppUpdater from './AppUpdater.svelte';
	import type { Loadable } from '@square/svelte-store';
	import type { Update } from '../../updater';
	import DomainButton from './DomainButton.svelte';
	import type { PrService } from '$lib/github/pullrequest';
	import type { BaseBranchService, VirtualBranchService } from '$lib/vbranches/branchStoresCache';
	import ProjectSelector from './ProjectSelector.svelte';
	import Branches from './Branches.svelte';
	import type { BranchService } from '$lib/remotecontributions/store';

	export let vbranchService: VirtualBranchService;
	export let branchService: BranchService;
	export let baseBranchService: BaseBranchService;
	export let branchController: BranchController;
	export let project: Project;
	export let user: User | undefined;
	export let update: Loadable<Update>;
	export let prService: PrService;
	export let projectService: ProjectService;

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
	<div class="flex h-8 flex-shrink-0" data-tauri-drag-region>
		<!-- Top spacer & drag region -->
	</div>
	<div class="relative mx-4 mb-4 mt-1">
		<ProjectSelector {project} {projectService} />
	</div>
	<div class="mx-4 mb-4 mt-1">
		<BaseBranchCard {project} {baseBranchService} {branchController} {prService} />
	</div>
	<div class="mb-4">
		<DomainButton href={`/${project.id}/board`} icon="branch">Applied branches</DomainButton>
	</div>
	<Branches projectId={project.id} {branchService} grow={!stashExpanded} />
	<YourBranches {project} {branchController} {vbranchService} bind:expanded={stashExpanded} />
	<Footer {user} projectId={project.id} />
	<AppUpdater {update} />
</div>

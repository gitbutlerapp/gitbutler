<script lang="ts">
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { User } from '$lib/backend/cloud';
	import BaseBranchCard from './BaseBranchCard.svelte';
	import type { Project, ProjectService } from '$lib/backend/projects';
	import Footer from './Footer.svelte';
	import AppUpdater from './AppUpdater.svelte';
	import { persisted, type Loadable } from '@square/svelte-store';
	import type { Update } from '../../updater';
	import DomainButton from './DomainButton.svelte';
	import type { PrService } from '$lib/github/pullrequest';
	import type { BaseBranchService } from '$lib/vbranches/branchStoresCache';
	import ProjectSelector from './ProjectSelector.svelte';
	import Branches from './Branches.svelte';
	import type { BranchService } from '$lib/branches/service';
	import Header from './Header.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import Tag from '../components/Tag.svelte';
	import * as toasts from '$lib/utils/toasts';
	import Resizer from '$lib/components/Resizer.svelte';
	import { getContext } from 'svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';

	export let branchService: BranchService;
	export let baseBranchService: BaseBranchService;
	export let branchController: BranchController;
	export let project: Project;
	export let user: User | undefined;
	export let update: Loadable<Update>;
	export let prService: PrService;
	export let projectService: ProjectService;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
	const defaultTrayWidthRem = persisted<number | undefined>(
		undefined,
		'defaulTrayWidth_ ' + project.id
	);

	$: base$ = baseBranchService.base$;

	let branchesExpanded = true;
	let viewport: HTMLDivElement;
</script>

<div
	class="navigation relative z-30 flex w-80 shrink-0 flex-col border-r"
	style:width={$defaultTrayWidthRem ? $defaultTrayWidthRem + 'rem' : null}
	bind:this={viewport}
	role="menu"
	tabindex="0"
>
	<div class="drag-region" data-tauri-drag-region>
		<Header />
	</div>
	<div class="domains">
		<ProjectSelector {project} {projectService} />
		<div class="flex flex-col gap-1">
			<BaseBranchCard {project} {baseBranchService} {branchController} {prService} />
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
				<span>Applied branches</span>
				{#if ($base$?.behind || 0) > 0}
					<Tooltip label="Merge upstream commits into common base">
						<Tag
							color="error"
							filled
							clickable
							on:click={async (e) => {
								e.preventDefault();
								e.stopPropagation();
								try {
									await branchController.updateBaseBranch();
								} catch {
									toasts.error('Failed update working directory');
								}
							}}
						>
							Update
						</Tag>
					</Tooltip>
				{/if}
			</DomainButton>
		</div>
	</div>
	<Branches projectId={project.id} {branchService} bind:expanded={branchesExpanded} />
	<Footer {user} projectId={project.id} />
	<AppUpdater {update} />
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
	}
	.drag-region {
		padding-top: var(--space-12);
		padding-left: var(--space-12);
		padding-right: var(--space-12);
	}
	.domains {
		padding-bottom: var(--space-24);
		padding-left: var(--space-12);
		padding-right: var(--space-12);
	}
</style>

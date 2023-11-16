<script lang="ts">
	import type { Project, ProjectService } from '$lib/backend/projects';
	import IconButton from '$lib/components/IconButton.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import type { PrService } from '$lib/github/pullrequest';
	import IconBranch from '$lib/icons/IconBranch.svelte';
	import IconDropDown from '$lib/icons/IconDropDown.svelte';
	import IconGithub from '$lib/icons/IconGithub.svelte';
	import IconRefresh from '$lib/icons/IconRefresh.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranchService } from '$lib/vbranches/branchStoresCache';
	import ProjectsPopup from './ProjectsPopup.svelte';

	export let project: Project;
	export let projectService: ProjectService;
	export let branchController: BranchController;
	export let baseBranchService: BaseBranchService;
	export let prService: PrService;

	$: base$ = baseBranchService.base$;

	$: projects$ = projectService.projects$;

	let popup: ProjectsPopup;
	let baseContents: HTMLElement;
	let fetching = false;
</script>

<div
	class="relative flex flex-col rounded-lg p-3"
	style:background-color="var(--bg-card)"
	bind:this={baseContents}
>
	<div class="flex flex-grow items-center">
		<div class="flex flex-grow items-center gap-1">
			<a href="/{project.id}/base" class="font-bold">{project.title}</a>
			{#if ($base$?.behind || 0) > 0}
				<Tooltip label="Unmerged upstream commits">
					<div
						class="flex h-4 w-4 items-center justify-center rounded-full bg-red-500 text-xs font-bold text-white"
					>
						{$base$?.behind}
					</div>
				</Tooltip>
			{/if}
		</div>
		<div class="flex gap-x-2">
			<IconButton
				class="items-center justify-center align-top "
				icon={IconDropDown}
				on:click={() => {
					popup.show();
				}}
			/>
			<IconButton
				class="items-center justify-center align-top "
				on:click={() => {
					fetching = true;
					branchController.fetchFromTarget().finally(() => (fetching = false));
					prService.reload();
				}}
			>
				<div class:animate-spin={fetching}>
					<IconRefresh class="h-4 w-4" />
				</div>
			</IconButton>
		</div>
	</div>
	<div class="flex flex-grow items-center text-sm">
		<div class="flex flex-grow items-center gap-1">
			{#if $base$?.remoteUrl.includes('github.com')}
				<IconGithub class="h-2.5 w-2.5" />
			{:else}
				<IconBranch class="h-2.5 w-2.5" />
			{/if}
			{$base$?.branchName}
		</div>
		<div>
			<Tooltip label="Last fetch from upstream">
				{#if $base$?.fetchedAt}
					<TimeAgo date={$base$.fetchedAt} />
				{/if}
			</Tooltip>
		</div>
	</div>
</div>
<ProjectsPopup bind:this={popup} projects={$projects$} />

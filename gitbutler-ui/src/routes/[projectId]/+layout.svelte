<script lang="ts">
	import type { LayoutData } from './$types';
	import { onMount } from 'svelte';
	import ProjectSetup from './ProjectSetup.svelte';
	import { unsubscribe } from '$lib/utils/random';
	import * as hotkeys from '$lib/utils/hotkeys';
	import Navigation from './navigation/Navigation.svelte';
	import Link from '$lib/components/Link.svelte';
	import Button from '$lib/components/Button.svelte';
	import { syncToCloud } from '$lib/backend/cloud';
	import { handleMenuActions } from '$lib/backend/menu_actions';
	import { subscribe as menuSubscribe } from '$lib/menu';
	import ProblemLoadingRepo from '$lib/components/ProblemLoadingRepo.svelte';
	import { getRemoteBranches } from '$lib/vbranches/branchStoresCache';

	export let data: LayoutData;

	$: projectService = data.projectService;
	$: branchController = data.branchController;
	$: updateService = data.updateService;
	$: githubService = data.githubService;
	$: vbranchService = data.vbranchService;
	$: branchesError$ = vbranchService.branchesError$;
	$: project$ = data.project$;
	$: branchService = data.branchService;
	$: userService = data.userService;
	$: projectId = data.projectId;

	$: baseBranchService = data.baseBranchService;
	$: baseBranch$ = baseBranchService.base$;
	$: gbBranchActive$ = data.gbBranchActive$;

	$: user$ = data.user$;

	let trayViewport: HTMLElement;
	handleMenuActions(data.projectId);

	onMount(() => {
		return unsubscribe(
			menuSubscribe(data.projectId),
			hotkeys.on('Meta+Shift+S', () => syncToCloud($project$?.id))
		);
	});
</script>

{#if $baseBranch$ === null}
	{#if $project$}
		{#await getRemoteBranches(projectId)}
			<p>loading...</p>
		{:then remoteBranches}
			{#if remoteBranches.length == 0}
				<ProblemLoadingRepo {userService} {projectService} project={$project$}>
					<p class="mt-6 text-red-500">You don't have any remote branches.</p>
					<p class="text-color-3 mt-6 text-sm">
						Currently, GitButler requires a remote branch to base it's virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base.
						<a
							target="_blank"
							rel="noreferrer"
							class="font-bold"
							href="https://docs.gitbutler.com/features/virtual-branches/butler-flow"
						>
							Learn more
						</a>
					</p>
				</ProblemLoadingRepo>
			{:else}
				<ProjectSetup {branchController} {userService} {projectId} {remoteBranches} />
			{/if}
		{/await}
	{/if}
{:else if !$gbBranchActive$}
	<ProblemLoadingRepo {userService} {projectService} project={$project$} alternativeArt>
		<svelte:fragment slot="title">
			Looks like you've switched from gitbutler/integration
		</svelte:fragment>

		Due to GitButler managing multiple virtual branches, you cannot switch back and forth between
		git branches and virtual branches easily.

		<Link href="https://docs.gitbutler.com/features/virtual-branches/integration-branch">
			Learn more
		</Link>

		<svelte:fragment slot="actions">
			<Button
				color="primary"
				icon="undo-small"
				on:click={() => {
					if ($baseBranch$) branchController.setTarget($baseBranch$.branchName);
				}}
			>
				Go back to gitbutler/integration
			</Button>
		</svelte:fragment>
	</ProblemLoadingRepo>
{:else if $branchesError$}
	<ProblemLoadingRepo {projectService} {userService} project={$project$}>
		{$branchesError$}
	</ProblemLoadingRepo>
{:else if $baseBranch$}
	<div class="relative flex w-full max-w-full" role="group" on:dragover|preventDefault>
		<div bind:this={trayViewport} class="z-30 flex flex-shrink">
			{#if $project$}
				<Navigation
					{branchService}
					{baseBranchService}
					{branchController}
					project={$project$}
					user={$user$}
					update={updateService.update$}
					{githubService}
					{projectService}
				/>
			{:else}
				<p>loading...</p>
			{/if}
		</div>
		<div class="absolute h-4 w-full" data-tauri-drag-region></div>
		<slot />
	</div>
{:else}
	loading...
{/if}

<script lang="ts">
	import { syncToCloud } from '$lib/backend/cloud';
	import { handleMenuActions } from '$lib/backend/menu_actions';
	import Navigation from '$lib/components/Navigation.svelte';
	import NotOnGitButlerBranch from '$lib/components/NotOnGitButlerBranch.svelte';
	import ProblemLoadingRepo from '$lib/components/ProblemLoadingRepo.svelte';
	import ProjectSetup from '$lib/components/ProjectSetup.svelte';
	import { subscribe as menuSubscribe } from '$lib/menu';
	import * as hotkeys from '$lib/utils/hotkeys';
	import { unsubscribe } from '$lib/utils/random';
	import { getRemoteBranches } from '$lib/vbranches/branchStoresCache';
	import { interval, Subscription } from 'rxjs';
	import { startWith, tap } from 'rxjs/operators';
	import { onMount } from 'svelte';
	import type { LayoutData } from './$types';
	import { page } from '$app/stores';

	export let data: LayoutData;

	$: projectService = data.projectService;
	$: branchController = data.branchController;
	$: githubService = data.githubService;
	$: vbranchService = data.vbranchService;
	$: branchesError$ = vbranchService.branchesError$;
	$: project$ = data.project$;
	$: branchService = data.branchService;
	$: userService = data.userService;
	$: projectId = data.projectId;

	$: baseBranchService = data.baseBranchService;
	$: baseBranch$ = baseBranchService.base$;
	$: baseError$ = baseBranchService.error$;
	$: gbBranchActive$ = data.gbBranchActive$;

	$: user$ = data.user$;

	let trayViewport: HTMLElement;
	handleMenuActions(data.projectId);

	let lastProjectId: string | undefined = undefined;
	onMount(() => {
		let fetchSub: Subscription;
		// Project is auto-fetched on page load and then every 15 minutes
		page.subscribe((page) => {
			if (page.params.projectId !== lastProjectId) {
				lastProjectId = page.params.projectId;
				fetchSub?.unsubscribe();
				fetchSub = interval(1000 * 60 * 15)
					.pipe(
						startWith(0),
						tap(() => baseBranchService.fetchFromTarget())
					)
					.subscribe();
			}
		});
		return unsubscribe(
			menuSubscribe(data.projectId),
			hotkeys.on('Meta+Shift+S', () => syncToCloud($project$?.id))
		);
	});
</script>

{#if !$project$}
	<p>Project not found!</p>
{:else if $baseError$}
	<ProblemLoadingRepo {projectService} {userService} project={$project$} error={$baseError$} />
{:else if $baseBranch$ === null}
	{@const remoteBranches = getRemoteBranches(projectId)}
	{#await remoteBranches}
		<p>loading...</p>
	{:then remoteBranches}
		{#if remoteBranches.length == 0}
			<ProblemLoadingRepo
				{userService}
				{projectService}
				project={$project$}
				error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
			/>
		{:else}
			<ProjectSetup {branchController} {userService} {projectId} {remoteBranches} />
		{/if}
	{:catch}
		<ProblemLoadingRepo
			{userService}
			{projectService}
			project={$project$}
			error="Currently, GitButler requires a remote branch to base its virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base"
		/>
	{/await}
{:else if $branchesError$}
	<ProblemLoadingRepo {projectService} {userService} project={$project$} error={$branchesError$} />
{:else if !$gbBranchActive$ && $baseBranch$}
	<NotOnGitButlerBranch
		{userService}
		{projectService}
		{branchController}
		project={$project$}
		baseBranch={$baseBranch$}
	/>
{:else if $baseBranch$}
	<div class="relative flex w-full max-w-full" role="group" on:dragover|preventDefault>
		<div bind:this={trayViewport} class="flex flex-shrink">
			<Navigation
				{branchService}
				{baseBranchService}
				{branchController}
				project={$project$}
				user={$user$}
				{githubService}
				{projectService}
			/>
		</div>
		<div class="absolute h-4 w-full" data-tauri-drag-region></div>
		<slot />
	</div>
{:else}
	loading...
{/if}

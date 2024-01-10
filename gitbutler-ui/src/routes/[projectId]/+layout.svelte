<script lang="ts">
	import type { LayoutData } from './$types';
	import { onMount } from 'svelte';
	import { Code } from '$lib/backend/ipc';
	import { goto } from '$app/navigation';
	import BaseBranchSelect from './BaseBranchSelect.svelte';
	import { unsubscribe } from '$lib/utils/random';
	import * as hotkeys from '$lib/utils/hotkeys';
	import Navigation from './navigation/Navigation.svelte';
	import Link from '$lib/components/Link.svelte';
	import Button from '$lib/components/Button.svelte';
	import { syncToCloud } from '$lib/backend/cloud';
	import { handleMenuActions } from '$lib/backend/menu_actions';
	import { subscribe as menuSubscribe } from '$lib/menu';

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
		<BaseBranchSelect {branchController} {userService} {projectId} />
	{/if}
{:else if $branchesError$}
	<div class="text-color-3 flex h-full w-full items-center justify-center">
		{#if $branchesError$.code === Code.ProjectHead}
			<div class="flex max-w-xl flex-col justify-center gap-y-3 p-4 text-center">
				<h2 class="text-lg font-semibold">Looks like you've switched from gitbutler/integration</h2>

				<p>
					Due to GitButler managing multiple virtual branches, you cannot switch back and forth
					between git branches and virtual branches easily.
				</p>

				<Link href="https://docs.gitbutler.com/features/virtual-branches/integration-branch">
					Learn more
				</Link>

				<div class="flex flex-col items-center">
					<Button
						color="primary"
						on:click={() => {
							if ($baseBranch$) branchController.setTarget($baseBranch$.branchName);
						}}
					>
						Go back to gitbutler/integration
					</Button>
				</div>
			</div>
		{:else}
			<div class="flex max-w-xl flex-col gap-y-3 p-4">
				<div>
					<h1 class="text-lg font-semibold">There was a problem loading this repo</h1>
					<p>{$branchesError$.message}</p>
				</div>
				<div class="text-center">
					<Button icon="home" on:click={() => goto('/')}>Home</Button>
				</div>
			</div>
		{/if}
	</div>
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

<script lang="ts">
	import type { LayoutData } from './$types';
	import { getContext, onMount } from 'svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { Code } from '$lib/backend/ipc';
	import Resizer from '$lib/components/Resizer.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import IconChevronLeft from '$lib/icons/IconChevronLeft.svelte';
	import { goto } from '$app/navigation';
	import BaseBranchSelect from './BaseBranchSelect.svelte';
	import { unsubscribe } from '$lib/utils/random';
	import * as hotkeys from '$lib/utils/hotkeys';
	import Navigation from './navigation/Navigation.svelte';
	import Link from '$lib/components/Link.svelte';
	import Button from '$lib/components/Button.svelte';
	import { syncToCloud } from '$lib/backend/cloud';
	import { handleMenuActions } from '$lib/backend/menu_actions';

	export let data: LayoutData;

	$: projectService = data.projectService;
	$: githubContext$ = data.githubContext$;
	$: branchController = data.branchController;
	$: update = data.update;
	$: prService = data.prService;
	$: vbranchService = data.vbranchService;
	$: branchesError$ = vbranchService.branchesError$;
	$: project$ = data.project$;
	$: branchService = data.branchService;

	$: baseBranchService = data.baseBranchService;
	$: baseBranch$ = baseBranchService.base$;

	$: remoteBranchService = data.remoteBranchService;
	$: user$ = data.user$;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	let trayViewport: HTMLElement;
	handleMenuActions(data.projectId);

	onMount(() => {
		return unsubscribe(hotkeys.on('Meta+Shift+S', () => syncToCloud($project$?.id)));
	});
</script>

{#if $branchesError$}
	{console.log($branchesError$)}
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
						color="purple"
						height="small"
						on:click={() => {
							if ($baseBranch$) branchController.setTarget($baseBranch$.branchName);
						}}
					>
						Go back to gitbutler/integration
					</Button>
				</div>
			</div>
		{:else}
			<div class="flex max-w-xl gap-x-2 p-4">
				<IconButton icon={IconChevronLeft} on:click={() => goto('/')}></IconButton>
				<div>
					<h1 class="text-lg font-semibold">There was a problem loading this repo</h1>
					<p>{$branchesError$.message}</p>
				</div>
			</div>
		{/if}
	</div>
{:else if $baseBranch$}
	<div class="relative flex w-full max-w-full" role="group" on:dragover|preventDefault>
		<div bind:this={trayViewport} class="z-30 flex flex-shrink">
			{#if $project$}
				<Navigation
					{vbranchService}
					{branchService}
					{baseBranchService}
					{branchController}
					project={$project$}
					user={$user$}
					{update}
					{prService}
					{projectService}
				/>
			{:else}
				<p>loading...</p>
			{/if}
		</div>
		<Resizer
			minWidth={300}
			viewport={trayViewport}
			direction="horizontal"
			class="z-30"
			on:width={(e) => {
				userSettings.update((s) => ({
					...s,
					trayWidth: e.detail
				}));
			}}
		/>
		<slot />
	</div>
{:else if $baseBranch$ === null}
	{#if $project$}
		<BaseBranchSelect projectId={$project$?.id} {project$} {projectService} {branchController} />
	{/if}
{:else}
	loading...
{/if}

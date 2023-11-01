<script lang="ts">
	import {
		IconGithub,
		IconGitBranch,
		IconLoading,
		IconTriangleDown,
		IconTriangleUp
	} from '$lib/icons';
	import { projectCreatePullRequestInsteadOfPush } from '$lib/config/config';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import { createEventDispatcher } from 'svelte';

	let expanded = false;
	let disabled = false;

	export let isLoading = false;
	export let projectId: string;
	export let githubContext: GitHubIntegrationContext | undefined;

	const dispatch = createEventDispatcher<{ trigger: { with_pr: boolean } }>();
	const createPr = projectCreatePullRequestInsteadOfPush(projectId);

	$: mode = $createPr ? 'pr' : 'push-only';
</script>

<div
	class="bg-color-3 border-color-3 absolute flex h-fit w-fit flex-col items-center whitespace-nowrap rounded border
				"
>
	<div class="flex h-6 flex-row items-center justify-center font-medium leading-5">
		<button
			{disabled}
			class="hover:bg-color-2 flex h-full items-center justify-center gap-1 rounded-l pl-2 pr-3"
			on:click={() => {
				dispatch('trigger', { with_pr: $createPr });
			}}
		>
			{#if isLoading}
				<IconLoading
					class="text-color-4 h-4 w-4 animate-spin fill-purple-600 dark:fill-purple-200"
				/>
			{:else if $createPr}
				<IconGithub class="text-color-4 h-4 w-4"></IconGithub>
			{:else}
				<IconGitBranch class="text-color-4 h-4 w-4"></IconGitBranch>
			{/if}

			{#if $createPr}
				<span>Create Pull Request</span>
			{:else}
				<span>Push to remote</span>
			{/if}
		</button>
		<button
			id="close-button"
			{disabled}
			class="hover:bg-color-2 flex h-full items-center justify-center rounded-r pl-1 pr-1.5"
			on:click={() => (expanded = !expanded)}
		>
			{#if expanded}
				<IconTriangleUp></IconTriangleUp>
			{:else}
				<IconTriangleDown></IconTriangleDown>
			{/if}
		</button>
	</div>
	{#if expanded}
		<div class="border-color-4 z-50 w-full border-t px-2 py-0.5">
			<label class="flex items-center gap-1">
				<input
					type="radio"
					name="mode"
					value="push-only"
					bind:group={mode}
					on:change={(e) => {
						if (e.target instanceof HTMLInputElement) {
							e.target.value === 'pr' ? createPr.set(true) : createPr.set(false);
							expanded = false;
						}
					}}
				/>
				Push to remote
			</label>
			{#if !githubContext}
				<Tooltip label="Setup GitHub integration in account settings to enable this feature">
					<label class="flex items-center gap-1">
						<input type="radio" disabled={true} />
						<span class={githubContext ? '' : 'text-color-3'}> Create Pull Request </span>
					</label>
				</Tooltip>
			{:else}
				<label class="flex items-center gap-1">
					<input
						type="radio"
						name="mode"
						value="pr"
						bind:group={mode}
						on:change={(e) => {
							if (e.target instanceof HTMLInputElement) {
								e.target.value === 'pr' ? createPr.set(true) : createPr.set(false);
								expanded = false;
							}
						}}
					/>
					<span class={githubContext ? '' : 'text-color-3'}> Create Pull Request </span>
				</label>
			{/if}
		</div>
	{/if}
</div>

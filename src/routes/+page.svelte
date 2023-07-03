<script lang="ts">
	import type { LayoutData } from './$types';
	import { Button, Tooltip } from '$lib/components';
	import { events } from '$lib';

	export let data: LayoutData;

	const { projects } = data;
</script>

{#await projects.load()}
	loading...
{:then}
	{#if $projects.length == 0}
		<div
			class="mx-auto flex h-full w-full items-center justify-center bg-light-200 p-8 text-light-900 dark:bg-dark-1000"
		>
			<div class="inline-block w-96 text-center">
				<svg
					class="mx-auto h-12 w-12 text-light-600 dark:text-dark-200"
					fill="none"
					viewBox="0 0 24 24"
					stroke="currentColor"
					aria-hidden="true"
				>
					<path
						vector-effect="non-scaling-stroke"
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M9 13h6m-3-3v6m-9 1V7a2 2 0 012-2h6l2 2h6a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z"
					/>
				</svg>
				<h3 class="mt-2 text-lg text-light-800 dark:font-bold dark:text-white">No projects</h3>
				<p class="mt-1 text-light-500 dark:text-dark-100">
					Get started by tracking a project you're working on.
				</p>
				<div class="mt-6">
					<Button color="primary" on:click={() => events.emit('openNewProjectModal')}>
						<span class="font-bold text-white">Start Tracking a Project</span>
					</Button>
				</div>
			</div>
		</div>
	{:else}
		<div class="h-full w-full items-center bg-light-200 p-8 text-light-900 dark:bg-dark-1000">
			<div class="mb-8 flex flex-row justify-between">
				<div class="text-light-900 dark:text-dark-100">
					<h1 class="mb-2 text-3xl">Your projects</h1>
					<p class="text-lg text-light-700 dark:text-dark-200">
						All the projects that I am currently assisting you with.
					</p>
				</div>
				<div class="self-start">
					<Button color="primary" on:click={() => events.emit('openNewProjectModal')}>
						Add project
					</Button>
				</div>
			</div>
			<div class="flex flex-wrap gap-4">
				{#each $projects as project}
					<a
						class="w-96 overflow-hidden rounded-lg bg-white text-light-900 shadow dark:border dark:border-dark-600 dark:bg-black dark:text-light-200"
						href="/repo/{project.id}/"
					>
						<div class="p-4">
							<h1 class="text-lg text-light-900 dark:text-dark-100">
								{project.title}
							</h1>
							<p class="text-light-700 dark:text-dark-200">
								{project.path}
							</p>
						</div>

						<div
							class="flex flex-row items-center gap-x-2 bg-light-100 px-4 py-2 text-light-900 dark:bg-dark-600 dark:text-dark-100"
						>
							{#if project.api}
								<div class="h-2 w-2 rounded-full bg-green-600" />
								<div>Backed-up</div>
							{:else}
								<div class="h-2 w-2 rounded-full bg-light-600 dark:bg-dark-200" />
								<div>Offline</div>
							{/if}
						</div>
					</a>
				{/each}
			</div>
		</div>
	{/if}
{/await}

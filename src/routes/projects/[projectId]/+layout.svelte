<script lang="ts">
	import type { LayoutData } from './$types';
	import { getContext } from 'svelte';
	import type { Writable } from 'svelte/store';
	import type { Project } from '$lib/projects';
	import { onDestroy } from 'svelte';
	import { page } from '$app/stores';
	import CommandPalette from '$lib/components/CommandPalette.svelte';

	export let data: LayoutData;

	$: project = data.project;

	function projectUrl(project: Project) {
		const gitUrl = project.api?.git_url;
		// get host from git url
		const url = new URL(gitUrl);
		const host = url.origin;
		const projectId = gitUrl.split('/').pop();

		return `${host}/projects/${projectId}`;
	}

	const contextProjectStore: Writable<Project | null | undefined> = getContext('project');
	$: contextProjectStore.set($project);
	onDestroy(() => {
		contextProjectStore.set(null);
	});

	$: selection = $page?.route?.id?.split('/')?.[3];
</script>

<div class="flex h-full w-full flex-col">
	<nav
		class="flex flex-none select-none items-center justify-between space-x-3 border-b border-zinc-700 py-1 px-8 text-zinc-300"
	>
		<div class="flex flex-row items-center space-x-2">
			<form action="/projects/{$project?.id}/search" method="GET" class="rounded-lg border-0 py-1.5 px-2 bg-zinc-800 ring-1 ring-inset ring-zinc-700">
				<div class="flex w-48 max-w-lg rounded-md shadow-sm">
					<div class="h-5 w-5 mr-2">
						<svg viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg"><path d="M8 12a4 4 0 110-8 4 4 0 010 8zm9.707 4.293l-4.82-4.82A5.968 5.968 0 0014 8 6 6 0 002 8a6 6 0 006 6 5.968 5.968 0 003.473-1.113l4.82 4.82a.997.997 0 001.414 0 .999.999 0 000-1.414z" fill="#5C5F62"/></svg>
					</div>
					<input
						type="text"
						name="search"
						id="search"
						placeholder="search"
						autocomplete="off"
						aria-label="Search input"
						class="block w-full min-w-0 flex-1 placeholder:text-zinc-500  text-zinc-200 sm:text-sm sm:leading-6 bg-zinc-800 focus:border-pink-400"
						style=""
					/>
					<span class="inline-flex items-center rounded border bg-zinc-700/50 border-zinc-700/20 px-1 text-gray-500 sm:text-sm shadow">
						&#8984;K
					</span>
				</div>
			</form>
			<a href="/projects/{$project?.id}/player" class="text-zinc-400 hover:text-zinc-200">
				<svg
					xmlns="http://www.w3.org/2000/svg"
					fill="none"
					viewBox="0 0 24 24"
					stroke-width="1.5"
					stroke="currentColor"
					class="h-6 w-6"
				>
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
					/>
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						d="M15.91 11.672a.375.375 0 010 .656l-5.603 3.113a.375.375 0 01-.557-.328V8.887c0-.286.307-.466.557-.327l5.603 3.112z"
					/>
				</svg>
			</a>
			<a href="/projects/{$project?.id}/timeline" class="text-orange-400 hover:text-zinc-200"
				>Timeline</a
			>
		</div>

		<ul>
			<li>
				<a href="/projects/{$project?.id}/settings" class="text-zinc-400 hover:text-zinc-300">
					<div class="rounded-md p-1 hover:bg-zinc-700 hover:bg-zinc-700 hover:text-zinc-200">
						<div class="h-6 w-6 ">
							<svg
								xmlns="http://www.w3.org/2000/svg"
								fill="none"
								viewBox="0 0 24 24"
								stroke-width="1.5"
								stroke="currentColor"
								class="h-6 w-6"
							>
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 010 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z"
								/>
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
								/>
							</svg>
						</div>
					</div>
				</a>
			</li>
		</ul>
	</nav>

	<div class="project-container flex-auto overflow-auto">
		<slot />
	</div>

	<footer class="w-full text-sm font-medium">
		<div
			class="flex h-6 flex-shrink-0 select-none items-center border-t border-zinc-700 bg-zinc-800 "
		>
			<div class="mx-4 flex w-full flex-row items-center justify-between space-x-2">
				{#if $project?.api?.sync}
					<a href="/projects/{$project?.id}/settings" class="text-zinc-400 hover:text-zinc-300">
						<div class="flex flex-row items-center space-x-2 ">
							<div class="h-2 w-2 rounded-full bg-green-700" />
							<div>Syncing</div>
						</div>
					</a>
					<a target="_blank" rel="noreferrer" href={projectUrl($project)}>Open in GitButler Cloud</a
					>
				{:else}
					<a href="/projects/{$project?.id}/settings" class="text-zinc-400 hover:text-zinc-300">
						<div class="flex flex-row items-center space-x-2 ">
							<div class="h-2 w-2 rounded-full bg-red-700" />
							<div>Offline</div>
						</div>
					</a>
				{/if}
			</div>
		</div>
	</footer>
</div>
<CommandPalette projectId={$project?.id} />

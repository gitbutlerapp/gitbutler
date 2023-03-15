<script lang="ts">
	import type { LayoutData } from './$types';
	import { getContext } from 'svelte';
	import type { Writable } from 'svelte/store';
	import type { Project } from '$lib/projects';
	import { onDestroy } from 'svelte';
	import { page } from '$app/stores';
	import { currentProject } from '$lib/current_project';
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';

	export let data: LayoutData;
	let query: string;
	const searchTerm = writable('');
	setContext('searchTerm', searchTerm);

	$: project = data.project;
	$: currentProject.set($project);

	const debounce = <T extends (...args: any[]) => any>(fn: T, delay: number) => {
		let timeout: ReturnType<typeof setTimeout>;
		return (...args: any[]) => {
			clearTimeout(timeout);
			timeout = setTimeout(() => fn(...args), delay);
		};
	};

	const updateQuery = debounce(async () => {
		searchTerm.set(query);
	}, 500);

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
			<form action="/projects/{$project?.id}/search" method="GET">
				<label
					for="default-search"
					class="sr-only mb-2 text-sm font-medium text-gray-900 dark:text-white">Search</label
				>
				<div class="relative">
					<div class="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-3">
						<svg class="mr-2 h-5 w-5" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg"
							><path
								d="M8 12a4 4 0 110-8 4 4 0 010 8zm9.707 4.293l-4.82-4.82A5.968 5.968 0 0014 8 6 6 0 002 8a6 6 0 006 6 5.968 5.968 0 003.473-1.113l4.82 4.82a.997.997 0 001.414 0 .999.999 0 000-1.414z"
								fill="#5C5F62"
							/></svg
						>
					</div>
					<div class="flex w-48 max-w-lg rounded-md shadow-sm">
						<input
							type="text"
							name="search"
							id="search"
							placeholder="search history"
							bind:value={query}
							on:input={updateQuery}
							autocomplete="off"
							aria-label="Search input"
							class="block w-full min-w-0 flex-1 rounded border border-zinc-700 bg-zinc-800  p-[3px] px-2 pl-10 text-zinc-200 placeholder:text-zinc-500 sm:text-sm sm:leading-6"
							style=""
						/>
					</div>
				</div>
			</form>
			<div
				class="right-1 top-1 inline-flex items-center rounded border border-zinc-700/20 bg-zinc-700/50 px-1 py-[2px] text-gray-400 shadow sm:text-sm"
			>
				&#8984;K
			</div>
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

	<div class="project-container h-100 flex-auto overflow-y-auto">
		<slot />
	</div>

	<footer class="w-full text-sm font-medium">
		<div class="flex h-8 flex-shrink-0 select-none items-center border-t border-zinc-700">
			<div class="mx-4 flex w-full flex-row items-center justify-between space-x-2">
				{#if $project?.api?.sync}
					<a href="/projects/{$project?.id}/settings" class="text-zinc-400 hover:text-zinc-300">
						<div class="flex flex-row items-center space-x-2 ">
							<div class="h-2 w-2 rounded-full bg-green-700" />
							<div>Backed up</div>
						</div>
					</a>
					<a target="_blank" rel="noreferrer" href={projectUrl($project)} class="flex">
						<div class="leading-5">Open in GitButler Cloud</div>
						<div class="icon ml-1 h-5 w-5">
							<svg viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg"
								><path
									fill="#52525B"
									d="M14 13v1a1 1 0 01-1 1H6c-.575 0-1-.484-1-1V7a1 1 0 011-1h1c1.037 0 1.04 1.5 0 1.5-.178.005-.353 0-.5 0v6h6V13c0-1 1.5-1 1.5 0zm-3.75-7.25A.75.75 0 0111 5h4v4a.75.75 0 01-1.5 0V7.56l-3.22 3.22a.75.75 0 11-1.06-1.06l3.22-3.22H11a.75.75 0 01-.75-.75z"
								/></svg
							>
						</div>
					</a>
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

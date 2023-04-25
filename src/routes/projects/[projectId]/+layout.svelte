<script lang="ts">
	import type { LayoutData } from './$types';
	import type { Project } from '$lib/api';
	import { Link, Tooltip } from '$lib/components';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { IconSearch, IconSettings, IconTerminal } from '$lib/components/icons';
	import { onMount } from 'svelte';
	import tinykeys from 'tinykeys';
	import { format } from 'date-fns';

	export let data: LayoutData;
	const { project } = data;

	let query: string;

	const onSearchSubmit = () => goto(`/projects/${$project?.id}/search?q=${query}`);

	function projectUrl(project: Project) {
		const gitUrl = project.api?.git_url;
		// get host from git url
		const url = new URL(gitUrl);
		const host = url.origin;
		const projectId = gitUrl.split('/').pop();

		return `${host}/projects/${projectId}`;
	}

	onMount(() =>
		tinykeys(window, {
			'Meta+Shift+C': () => $project && goto(`/projects/${$project.id}/commit/`),
			'Meta+T': () => $project && goto(`/projects/${$project.id}/terminal/`),
			'Meta+P': () => $project && goto(`/projects/${$project.id}/`),
			'Meta+Shift+,': () => $project && goto(`/projects/${$project.id}/settings/`),
			'Meta+R': () =>
				$project && goto(`/projects/${$project.id}/player/${format(new Date(), 'yyyy-MM-dd')}`),
			'a i p': () => $project && goto(`/projects/${$project.id}/aiplayground/`)
		})
	);

	$: selection = $page?.route?.id?.split('/')?.[3];
</script>

<div class="flex h-full w-full flex-col">
	{#if selection !== 'player'}
		<nav
			class="flex flex-none select-none items-center justify-between space-x-3 border-b border-zinc-700 py-1 px-2 text-zinc-300"
		>
			<div class="flex flex-row items-center space-x-2">
				<form action="/projects/{$project?.id}/search" method="GET">
					<label
						for="default-search"
						class="text-gray-900 sr-only mb-2 text-sm font-medium dark:text-white">Search</label
					>
					<div class="relative">
						<div class="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-3">
							<IconSearch class="h-5 w-5 text-zinc-500" />
						</div>
						<form
							on:submit|preventDefault={onSearchSubmit}
							class="flex w-48 max-w-lg rounded-md shadow-sm"
						>
							<input type="submit" class="hidden" />
							<input
								type="text"
								name="search"
								id="search"
								placeholder="search history"
								bind:value={query}
								autocomplete="off"
								aria-label="Search input"
								class="block w-full min-w-0 flex-1 rounded border border-zinc-700 bg-zinc-800  p-[3px] px-2 pl-10 text-zinc-200 placeholder:text-zinc-500 sm:text-sm sm:leading-6"
							/>
						</form>
					</div>
				</form>
				<div
					class="right-1 top-1 inline-flex items-center rounded border border-zinc-700/20 bg-zinc-700/50 px-1 py-[2px] font-mono text-zinc-200 shadow sm:text-sm"
				>
					&#8984;&nbsp;K
				</div>
			</div>

			<ul class="flex gap-2">
				<li>
					<Tooltip label="Terminal">
						<a
							class="block rounded p-1 text-zinc-400 hover:bg-zinc-700 hover:text-zinc-200"
							href="/projects/{$project?.id}/terminal"
						>
							<IconTerminal class="h-6 w-6" />
						</a>
					</Tooltip>
				</li>
				<li>
					<Tooltip label="Project settings">
						<a
							class="block rounded p-1 text-zinc-400 hover:bg-zinc-700 hover:text-zinc-200"
							href="/projects/{$project?.id}/settings"
						>
							<IconSettings class="h-6 w-6" />
						</a>
					</Tooltip>
				</li>
			</ul>
		</nav>
	{/if}

	<div class="project-container h-100 flex-auto overflow-y-auto">
		<slot />
	</div>

	<footer class="w-full text-sm font-medium">
		<div class="flex h-8 flex-shrink-0 select-none items-center border-t border-zinc-700">
			<div class="mx-4 flex w-full flex-row items-center justify-between space-x-2">
				{#if $project?.api?.sync}
					<Link href="/projects/{$project?.id}/settings">
						<div class="flex flex-row items-center space-x-2 ">
							<div class="h-2 w-2 rounded-full bg-green-700" />
							<div>Backed up</div>
						</div>
					</Link>
					<Link target="_blank" rel="noreferrer" href={projectUrl($project)}>
						Open in GitButler Cloud
					</Link>
				{:else}
					<Link href="/projects/{$project?.id}/settings">
						<div class="h-2 w-2 rounded-full bg-red-700" />
						Offline
					</Link>
				{/if}
			</div>
		</div>
	</footer>
</div>

<script lang="ts">
	import type { LayoutData } from './$types';
	import type { Project } from '$lib/api';
	import { Button, Link, Tooltip } from '$lib/components';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { IconEmail, IconRewind, IconSearch, IconSettings, IconTerminal } from '$lib/icons';
	import { PUBLIC_API_BASE_URL } from '$env/static/public';
	import { events } from '$lib';

	export let data: LayoutData;
	const { project } = data;

	let query: string;

	const onSearchSubmit = () =>
		goto(`/projects/${$project?.id}/search?q=${encodeURIComponent(query)}`);

	const projectUrl = (project: Project) =>
		new URL(`/projects/${project.id}`, new URL(PUBLIC_API_BASE_URL)).toString();

	$: selection = $page?.route?.id?.split('/')?.[3];
</script>

<div class="flex h-full w-full flex-col">
	{#if selection !== 'player'}
		<nav
			class="flex flex-none select-none items-center justify-between space-x-3 border-b border-light-100 px-2 py-1 text-dark-300 dark:border-dark-700 dark:text-light-300"
		>
			<div class="flex flex-row items-center space-x-2">
				<form action="/projects/{$project?.id}/search" method="GET">
					<label
						for="default-search"
						class="texxt-dark-900 sr-only mb-2 text-sm font-medium dark:text-light-900"
						>Search</label
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
								autocomplete="off"
								autocorrect="off"
								spellcheck="true"
								type="text"
								name="search"
								id="search"
								placeholder="Search history"
								bind:value={query}
								aria-label="Search input"
								class="block w-full min-w-0 flex-1 rounded border border-light-600 bg-light-800 p-[3px] px-2 pl-10 text-dark-200 placeholder:text-dark-300 dark:border-dark-600 dark:bg-dark-800 dark:text-light-200 dark:placeholder:text-light-300 sm:leading-6"
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

			<ul class="app-page-navigation flex gap-2">
				<li>
					<Tooltip label="Replay">
						<Button
							on:click={() => goto(`/projects/${$project.id}/player`)}
							kind="plain"
							icon={IconRewind}
						/>
					</Tooltip>
				</li>
				<li>
					<Tooltip label="Terminal">
						<Button
							on:click={() => goto(`/projects/${$project.id}/terminal`)}
							kind="plain"
							icon={IconTerminal}
						/>
					</Tooltip>
				</li>
				<li>
					<Tooltip label="Project settings">
						<Button
							on:click={() => goto(`/projects/${$project.id}/settings`)}
							kind="plain"
							icon={IconSettings}
						/>
					</Tooltip>
				</li>
			</ul>
		</nav>
	{/if}

	<div class="project-container h-100 flex-auto overflow-y-auto">
		<slot />
	</div>
</div>

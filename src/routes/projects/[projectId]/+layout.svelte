<script lang="ts">
	import type { LayoutData } from './$types';
	import type { Project } from '$lib/api';
	import { Button, Link, Tooltip } from '$lib/components';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { IconEmail, IconSearch, IconSettings, IconTerminal } from '$lib/icons';
	import QuickCommitModal from './QuickCommitModal.svelte';
	import { onMount } from 'svelte';
	import { unsubscribe } from '$lib/utils';
	import { PUBLIC_API_BASE_URL } from '$env/static/public';
	import { events, hotkeys, stores } from '$lib';

	export let data: LayoutData;
	const { cloud, head, statuses, diffs } = data;

	$: project = stores.project({ id: $page.params.projectId });
	const user = stores.user;

	let query: string;

	const onSearchSubmit = () => goto(`/projects/${$project?.id}/search?q=${query}`);

	const projectUrl = (project: Project) =>
		new URL(`/projects/${project.id}`, new URL(PUBLIC_API_BASE_URL)).toString();

	$: selection = $page?.route?.id?.split('/')?.[3];

	let quickCommitModal: QuickCommitModal;
	onMount(() =>
		unsubscribe(
			events.on('openQuickCommitModal', () => quickCommitModal?.show()),

			hotkeys.on('C', () => events.emit('openQuickCommitModal')),
			hotkeys.on('Meta+Shift+C', () => $project && goto(`/projects/${$project.id}/commit/`)),
			hotkeys.on('Meta+T', () => $project && goto(`/projects/${$project.id}/terminal/`)),
			hotkeys.on('Meta+P', () => $project && goto(`/projects/${$project.id}/`)),
			hotkeys.on('Meta+Shift+,', () => $project && goto(`/projects/${$project.id}/settings/`)),
			hotkeys.on('Meta+R', () => $project && goto(`/projects/${$project.id}/player/`)),
			hotkeys.on('a i p', () => $project && goto(`/projects/${$project.id}/aiplayground/`))
		)
	);
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
								autocomplete="off"
								autocorrect="off"
								spellcheck="true"
								type="text"
								name="search"
								id="search"
								placeholder="Search history"
								bind:value={query}
								aria-label="Search input"
								class="block w-full min-w-0 flex-1 rounded border border-zinc-700 bg-zinc-800  p-[3px] px-2 pl-10 text-zinc-200 placeholder:text-zinc-500 sm:leading-6"
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
						<Button
							on:click={() => $project && goto(`/projects/${$project.id}/terminal`)}
							kind="plain"
							icon={IconTerminal}
						/>
					</Tooltip>
				</li>
				<li>
					<Tooltip label="Project settings">
						<Button
							on:click={() => $project && goto(`/projects/${$project.id}/settings`)}
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

	<footer class="w-full text-sm font-medium">
		<div class="flex h-8 flex-shrink-0 select-none items-center border-t border-zinc-700">
			<div class="mx-4 flex w-full flex-row items-center justify-between space-x-2">
				<Link href="/projects/{$project?.id}/settings">
					{#if $project?.api?.sync}
						<div class="flex flex-row items-center space-x-2 ">
							<div class="h-2 w-2 rounded-full bg-green-700" />
							<span>Backed up</span>
						</div>
					{:else}
						<div class="h-2 w-2 rounded-full bg-red-700" />
						Offline
					{/if}
				</Link>

				<div class="flex gap-1">
					<Tooltip label="Send feedback">
						<Button
							kind="plain"
							height="small"
							icon={IconEmail}
							on:click={() => events.emit('openSendIssueModal')}
						/>
					</Tooltip>

					{#if $project?.api?.sync}
						<Link target="_blank" rel="noreferrer" href={projectUrl($project)}>
							Open in GitButler Cloud
						</Link>
					{/if}
				</div>
			</div>
		</div>
	</footer>
</div>

{#await Promise.all([diffs.load(), user.load(), project.load(), statuses.load()]) then}
	{#if $user && $project}
		<QuickCommitModal
			bind:this={quickCommitModal}
			user={$user}
			{cloud}
			project={$project}
			head={$head}
			diffs={$diffs}
			statuses={$statuses}
		/>
	{/if}
{/await}

<script lang="ts">
	import type { Project } from '$lib/projects';
	import type { Session } from '$lib/sessions';
	import { toHumanReadableTime } from '$lib/time';
	import { getContext } from 'svelte';
	import type { Writable } from 'svelte/store';
	import { IconHome, IconChevronRight } from '@tabler/icons-svelte';
	import Popover from '$lib/components/Popover';

	let project: Writable<Project | null | undefined> = getContext('project');
	let session: Writable<Session | null | undefined> = getContext('session');
	let projects: Writable<any> = getContext('projects');
</script>

<div class="flex flex-row items-center space-x-1 bg-zinc-900 text-zinc-400 h-8">
	<a class="hover:text-zinc-200" href="/">
		<IconHome class="w-5 h-5" />
	</a>
	{#if $project}
		<IconChevronRight class="w-5 h-5 text-zinc-700" />
		<div class="flex flex-col">
			<Popover>
				<div slot="button">
					{$project.title}
				</div>
				<div class="flex flex-col space-y-2">
					<ul class="flex flex-col space-y-2">
						{#each $projects || [] as project}
							<a
								href="/projects/{project.id}"
								class="p-2 rounded hover:bg-zinc-700 cursor-pointer truncate"
							>
								{project.title}</a
							>
						{/each}
					</ul>
					<span class="w-full border-t border-zinc-700" />
					<!-- svelte-ignore a11y-click-events-have-key-events -->
					<div class="p-2 rounded hover:bg-zinc-700 cursor-pointer">Add project...</div>
				</div>
			</Popover>
		</div>
	{/if}
	{#if $project && $session}
		<IconChevronRight class="w-5 h-5 text-zinc-700" />
		<a class="hover:text-zinc-200" href="/projects/{$project.id}/sessions/{$session.id}">
			{toHumanReadableTime($session.meta.startTimestampMs)}
			{toHumanReadableTime($session.meta.lastTimestampMs)}
		</a>
	{/if}
</div>

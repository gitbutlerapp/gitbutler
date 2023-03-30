<script lang="ts">
	import type { Project } from '$lib/projects';
	import type { Session } from '$lib/sessions';
	import { toHumanReadableTime } from '$lib/time';
	import { getContext } from 'svelte';
	import type { Writable } from 'svelte/store';
	import Popover from '$lib/components/Popover';

	let project: Writable<Project | null | undefined> = getContext('project');
	let session: Writable<Session | null | undefined> = getContext('session');
	let projects: Writable<any> = getContext('projects');
</script>

<div class="flex flex-row items-center text-zinc-400">
	<a
		Title="GitButler home"
		class="button-home group rounded-md p-2 hover:bg-zinc-700 hover:text-zinc-200 cursor-default"
		href="/"
	>
		<div class="flex h-4 w-4 items-center justify-center">
			<svg
				width="14"
				height="14"
				viewBox="0 0 14 14"
				class="group-hover:fill-zinc-50"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path
					d="M7.5547 0.16795C7.2188 -0.0559832 6.7812 -0.0559832 6.4453 0.16795L0.8906 3.87108C0.334202 4.24201 0 4.86648 0 5.53518V12C0 13.1046 0.895431 14 2 14H4C4.55228 14 5 13.5523 5 13V9H9V13C9 13.5523 9.44771 14 10 14H12C13.1046 14 14 13.1046 14 12V5.53518C14 4.86648 13.6658 4.24202 13.1094 3.87108L7.5547 0.16795Z"
					fill="#5C5F62"
					class="group-hover:fill-zinc-300"
				/>
			</svg>
		</div>
	</a>
	{#if $project}
		<div class="ml-1">
			<div Title="Project" class="project-home-button flex rounded-md py-2 px-2 hover:bg-zinc-700">
				<a class="flex h-4 items-center cursor-default" href={`/projects/${$project.id}`}>
					{$project.title}
				</a>
			</div>
		</div>
	{/if}
	{#if $project && $session}
		<a class="hover:text-zinc-200" href="/projects/{$project.id}/sessions/{$session.id}">
			{toHumanReadableTime($session.meta.startTimestampMs)}
			{toHumanReadableTime($session.meta.lastTimestampMs)}
		</a>
	{/if}
</div>

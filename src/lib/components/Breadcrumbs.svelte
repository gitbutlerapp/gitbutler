<script lang="ts">
	import type { Project } from '$lib/projects';
	import type { Session } from '$lib/sessions';
	import { toHumanReadableTime } from '$lib/time';
	import { getContext } from 'svelte';
	import type { Writable } from 'svelte/store';
	import { IconHome, IconChevronRight } from '@tabler/icons-svelte';

	let project: Writable<Project | null | undefined> = getContext('project');
	let session: Writable<Session | null | undefined> = getContext('session');
</script>

<div class="flex flex-row items-center space-x-1 bg-zinc-900 text-zinc-400 h-8">
	<a class="hover:text-zinc-200" href="/">
		<IconHome class="w-5 h-5" />
	</a>
	{#if $project}
		<IconChevronRight class="w-5 h-5 text-zinc-700" />
		<a class="hover:text-zinc-200" href="/projects/{$project.id}">{$project.title}</a>
	{/if}
	{#if $project && $session}
		<IconChevronRight class="w-5 h-5 text-zinc-700" />
		<a class="hover:text-zinc-200" href="/projects/{$project.id}/sessions/{$session.id}">
			{toHumanReadableTime($session.meta.startTimestampMs)}
			{toHumanReadableTime($session.meta.lastTimestampMs)}
		</a>
	{/if}
</div>

<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import { format } from 'date-fns';

	export let data: PageData;
	const { sessions, projectId } = data;

	$: dateSessions = $sessions.filter(
		(session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === $page.params.date
	);

	$: firstSession = dateSessions[dateSessions.length - 1];

	onMount(() =>
		goto(`/projects/${projectId}/player/${$page.params.date}/${firstSession.id}${$page.url.search}`)
	);
</script>

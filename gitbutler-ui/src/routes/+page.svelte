<script lang="ts">
	import AnalyticsConfirmation from '$lib/components/AnalyticsConfirmation.svelte';
	import DecorativeSplitView from '$lib/components/DecorativeSplitView.svelte';
	import FullscreenLoading from '$lib/components/FullscreenLoading.svelte';
	import Welcome from '$lib/components/Welcome.svelte';
	import { appAnalyticsConfirmed } from '$lib/config/appSettings';
	import { map } from 'rxjs';
	import type { PageData } from './$types';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	export let data: PageData;

	const { projectService, userService } = data;
	const projects$ = projectService.projects$;
	$: user$ = userService.user$;
	$: debug = $page.url.searchParams.get('debug');

	const analyticsConfirmed = appAnalyticsConfirmed();
	const persistedId = projectService.getLastOpenedProject();
	const redirect$ = projects$.pipe(
		map((projects) => {
			if (debug) return null;
			const projectId = projects.find((p) => p.id == persistedId)?.id;
			if (projectId) return projectId;
			if (projects.length > 0) return projects[0].id;
			return null;
		})
	);

	$: if ($redirect$) goto(`/${$redirect$}/`);
</script>

{#if $redirect$ === undefined}
	<FullscreenLoading />
{:else if !$analyticsConfirmed}
	<DecorativeSplitView
		user={$user$}
		imgSet={{
			light: '/images/img_analytics-light.webp',
			dark: '/images/img_analytics-dark.webp'
		}}
	>
		<AnalyticsConfirmation {analyticsConfirmed} />
	</DecorativeSplitView>
{:else if $redirect$ === null}
	<DecorativeSplitView
		user={$user$}
		imgSet={{
			light: '/images/img_moon-door-light.webp',
			dark: '/images/img_moon-door-dark.webp'
		}}
	>
		<Welcome {projectService} {userService} />
	</DecorativeSplitView>
{/if}

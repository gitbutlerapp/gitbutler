<script lang="ts">
	import { goto } from "$app/navigation";
	import AnalyticsConfirmation from "$components/onboarding/AnalyticsConfirmation.svelte";
	import Welcome from "$components/onboarding/Welcome.svelte";
	import IllustrationSplitLayout from "$components/shared/IllustrationSplitLayout.svelte";
	import newProjectSvg from "$lib/assets/illustrations/new-project.svg?raw";
	import newZenSvg from "$lib/assets/illustrations/new-zen.svg?raw";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { sleep } from "$lib/utils/sleep";
	import { inject } from "@gitbutler/core/context";
	import { TestId } from "@gitbutler/ui";

	const settingsService = inject(SETTINGS_SERVICE);
	const appSettings = $derived(settingsService.appSettings);

	const analyticsConfirmed = $derived($appSettings?.onboardingComplete);

	const projectsService = inject(PROJECTS_SERVICE);
	const projectsQuery = $derived(projectsService.projects());

	// We don't want to have this guard in the layout, because we want to have
	// `/onboarding/clone` accessible.
	$effect(() => {
		// Users should not be able to get here now that we load projects
		// sensibly, but hey, let's be sure.
		if (projectsQuery.response && projectsQuery.response.length > 0) {
			sleep(50).then(() => {
				goto("/");
			});
		}
	});
</script>

{#if analyticsConfirmed === undefined}
	<!-- Loading state while we determine if analytics have been confirmed -->
	loading...
{:else}
	<IllustrationSplitLayout
		img={analyticsConfirmed ? newZenSvg : newProjectSvg}
		testId={TestId.OnboardingPage}
	>
		{#if analyticsConfirmed}
			<Welcome />
		{:else}
			<AnalyticsConfirmation />
		{/if}
	</IllustrationSplitLayout>
{/if}

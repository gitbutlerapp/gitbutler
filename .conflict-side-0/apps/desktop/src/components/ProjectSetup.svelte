<script lang="ts">
	import { goto } from '$app/navigation';
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import ProjectSetupTarget from '$components/ProjectSetupTarget.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { OnboardingEvent, POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import newZenSvg from '$lib/assets/illustrations/new-zen.svg?raw';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/core/context';
	import { TestId } from '@gitbutler/ui';
	import type { RemoteBranchInfo } from '$lib/baseBranch/baseBranch';

	interface Props {
		projectId: string;
		remoteBranches: RemoteBranchInfo[];
	}

	const { projectId, remoteBranches }: Props = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const baseService = inject(BASE_BRANCH_SERVICE);
	const posthog = inject(POSTHOG_WRAPPER);
	const projectQuery = $derived(projectsService.getProject(projectId));
	const [setBaseBranchTarget] = baseService.setTarget;

	async function setTarget(branch: string[]) {
		if (!branch[0] || branch[0] === '') return;

		try {
			await setBaseBranchTarget({
				projectId: projectId,
				branch: branch[0],
				pushRemote: branch[1]
			});
			posthog.captureOnboarding(OnboardingEvent.SetTargetBranch);
			goto(`/${projectId}/`, { invalidateAll: true });
		} catch (e: unknown) {
			posthog.captureOnboarding(OnboardingEvent.SetTargetBranchFailed, e);
		}
	}

	$effect(() => {
		if (projectQuery.result.isError) {
			console.error('Failed to load project, redirecting:', projectQuery.result.error);
			goto('/');
		}
	});
</script>

<DecorativeSplitView img={newZenSvg} testId={TestId.ProjectSetupPage}>
	<ReduxResult {projectId} result={projectQuery.result}>
		{#snippet children(project)}
			<ProjectSetupTarget
				{projectId}
				projectName={project.title}
				{remoteBranches}
				onBranchSelected={async (branch) => {
					await setTarget(branch);
				}}
			/>
		{/snippet}
	</ReduxResult>
</DecorativeSplitView>

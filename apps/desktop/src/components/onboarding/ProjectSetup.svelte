<script lang="ts">
	import { goto } from "$app/navigation";
	import ProjectSetupTarget from "$components/onboarding/ProjectSetupTarget.svelte";
	import IllustrationSplitLayout from "$components/shared/IllustrationSplitLayout.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import newZenSvg from "$lib/assets/illustrations/new-zen.svg?raw";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { showError } from "$lib/error/showError";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { OnboardingEvent, POSTHOG_WRAPPER } from "$lib/telemetry/posthog";
	import { inject } from "@gitbutler/core/context";
	import { TestId } from "@gitbutler/ui";
	import type { RemoteBranchInfo } from "$lib/baseBranch/baseBranch";

	interface Props {
		projectId: string;
		remoteBranches: RemoteBranchInfo[];
	}

	const { projectId, remoteBranches }: Props = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const baseService = inject(BASE_BRANCH_SERVICE);
	const posthog = inject(POSTHOG_WRAPPER);
	const settingsStore = inject(SETTINGS_SERVICE).appSettings;
	const projectQuery = $derived(projectsService.getProject(projectId));
	const [setBaseBranchTarget] = baseService.setTarget;
	const [setBaseBranchTargetRef] = baseService.setTargetRef;

	async function setTarget([branchName, pushRemote]: [branchName: string, pushRemote: string]) {
		if (!branchName) return;

		try {
			if ($settingsStore?.featureFlags.singleBranch) {
				// Only set the target; the user keeps working on their current branch.
				await setBaseBranchTargetRef({
					projectId: projectId,
					targetRef: `refs/remotes/${branchName}`,
					pushRemote,
				});
			} else {
				await setBaseBranchTarget({
					projectId: projectId,
					branch: branchName,
					pushRemote,
				});
			}
		} catch (e: unknown) {
			posthog.captureOnboarding(OnboardingEvent.SetTargetBranchFailed, e);
			throw e;
		}

		posthog.captureOnboarding(OnboardingEvent.SetTargetBranch);
	}

	async function openProject(): Promise<boolean> {
		const destination = `/${projectId}/`;
		try {
			await goto(destination, { invalidateAll: true });
			return true;
		} catch (error) {
			showError("The target was set, but the project could not be opened", error);
			return false;
		}
	}

	$effect(() => {
		if (projectQuery.result.isError) {
			console.error("Failed to load project, redirecting:", projectQuery.result.error);
			goto("/");
		}
	});
</script>

<IllustrationSplitLayout img={newZenSvg} testId={TestId.ProjectSetupPage}>
	<ReduxResult {projectId} result={projectQuery.result}>
		{#snippet children(project)}
			<ProjectSetupTarget
				{projectId}
				projectName={project.title}
				{remoteBranches}
				onBranchSelected={setTarget}
				onOpenProject={openProject}
			/>
		{/snippet}
	</ReduxResult>
</IllustrationSplitLayout>

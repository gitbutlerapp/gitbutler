<script lang="ts">
	import { goto } from "$app/navigation";
	import ProjectSetupTarget from "$components/onboarding/ProjectSetupTarget.svelte";
	import IllustrationSplitLayout from "$components/shared/IllustrationSplitLayout.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import newZenSvg from "$lib/assets/illustrations/new-zen.svg?raw";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { showError } from "$lib/error/showError";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { OnboardingEvent, POSTHOG_WRAPPER } from "$lib/telemetry/posthog";
	import { inject } from "@gitbutler/core/context";
	import { TestId } from "@gitbutler/ui-svelte";
	import type { RemoteBranchInfo } from "$lib/baseBranch/baseBranch";
	import type { LocalTargetBranch } from "$lib/branches/branchEndpoints";

	interface Props {
		projectId: string;
		remoteBranches: RemoteBranchInfo[];
		localBranches: LocalTargetBranch[];
		/** Why the target needs choosing again, if it was set before. */
		notice?: string;
	}

	const { projectId, remoteBranches, localBranches, notice }: Props = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const baseService = inject(BASE_BRANCH_SERVICE);
	const posthog = inject(POSTHOG_WRAPPER);
	const projectQuery = $derived(projectsService.getProject(projectId));
	const [setBaseBranchTargetRef] = baseService.setTargetRef;

	async function setTarget([branchName, pushRemote]: [
		branchName: string,
		pushRemote: string | undefined,
	]) {
		if (!branchName) return;

		try {
			// Only set the target; the user keeps working on their current branch.
			await setBaseBranchTargetRef({
				projectId,
				targetRef: branchName,
				pushRemote,
			});
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
				{localBranches}
				onBranchSelected={setTarget}
				onOpenProject={openProject}
				{notice}
			/>
		{/snippet}
	</ReduxResult>
</IllustrationSplitLayout>

<script lang="ts">
	import { goto } from '$app/navigation';
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import KeysForm from '$components/KeysForm.svelte';
	import ProjectSetupTarget from '$components/ProjectSetupTarget.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { platformName } from '$lib/platform/platform';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { RemoteBranchInfo } from '$lib/baseBranch/baseBranch';

	interface Props {
		projectId: string;
		remoteBranches: RemoteBranchInfo[];
	}

	const { projectId, remoteBranches }: Props = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const baseService = inject(BASE_BRANCH_SERVICE);
	const posthog = inject(POSTHOG_WRAPPER);
	const projectResult = $derived(projectsService.getProject(projectId));
	const [setBaseBranchTarget, settingBranch] = baseService.setTarget;

	let selectedBranch = $state(['', '']);

	async function setTarget() {
		if (!selectedBranch[0] || selectedBranch[0] === '') return;

		try {
			await setBaseBranchTarget({
				projectId: projectId,
				branch: selectedBranch[0],
				pushRemote: selectedBranch[1]
			});
			goto(`/${projectId}/`, { invalidateAll: true });
		} finally {
			posthog.capture('Project Setup Complete');
		}
	}
</script>

<DecorativeSplitView img={newProjectSvg} testId={TestId.ProjectSetupPage}>
	{#if selectedBranch[0] && selectedBranch[0] !== '' && platformName !== 'windows'}
		{@const [remoteName, branchName] = selectedBranch[0].split(/\/(.*)/s)}
		<KeysForm {projectId} {remoteName} {branchName} disabled={settingBranch.current.isLoading} />
		<div class="actions">
			<Button
				kind="outline"
				disabled={settingBranch.current.isLoading}
				onclick={() => (selectedBranch[0] = '')}
			>
				Back
			</Button>
			<Button
				style="pop"
				loading={settingBranch.current.isLoading}
				onclick={setTarget}
				testId={TestId.ProjectSetupGitAuthPageButton}>Let's go!</Button
			>
		</div>
	{:else}
		<ReduxResult {projectId} result={projectResult.current}>
			{#snippet children(project)}
				<ProjectSetupTarget
					{projectId}
					projectName={project.title}
					{remoteBranches}
					onBranchSelected={async (branch) => {
						selectedBranch = branch;
						if (platformName !== 'windows') return;
						setTarget();
					}}
				/>
			{/snippet}
		</ReduxResult>
	{/if}
</DecorativeSplitView>

<style lang="postcss">
	.actions {
		margin-top: 20px;
		text-align: right;
	}
</style>

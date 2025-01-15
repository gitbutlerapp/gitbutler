<script lang="ts">
	import ProjectSetupTarget from './ProjectSetupTarget.svelte';
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import KeysForm from '$components/KeysForm.svelte';
	import { PostHogWrapper } from '$lib/analytics/posthog';
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import { BaseBranchService, type RemoteBranchInfo } from '$lib/baseBranch/baseBranchService';
	import { platformName } from '$lib/platform/platform';
	import { Project, ProjectsService } from '$lib/project/projects';
	import { BranchController } from '$lib/vbranches/branchController';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';

	interface Props {
		remoteBranches: RemoteBranchInfo[];
	}

	const { remoteBranches }: Props = $props();

	const project = getContext(Project);
	const projectsService = getContext(ProjectsService);
	const branchController = getContext(BranchController);
	const baseBranchService = getContext(BaseBranchService);
	const posthog = getContext(PostHogWrapper);

	let selectedBranch = $state(['', '']);
	let loading = $state(false);

	async function setTarget() {
		if (!selectedBranch[0] || selectedBranch[0] === '') return;

		loading = true;
		try {
			// TODO: Refactor temporary solution to forcing Windows to use system executable
			if (platformName === 'windows') {
				project.preferred_key = 'systemExecutable';
				await projectsService.updateProject(project);
				await baseBranchService.refresh();
			}
			await branchController.setTarget(selectedBranch[0], selectedBranch[1]);
			goto(`/${project.id}/`, { invalidateAll: true });
		} finally {
			posthog.capture('Project Setup Complete');
			loading = false;
		}
	}
</script>

<DecorativeSplitView img={newProjectSvg}>
	{#if selectedBranch[0] && selectedBranch[0] !== '' && platformName !== 'windows'}
		{@const [remoteName, branchName] = selectedBranch[0].split(/\/(.*)/s)}
		<KeysForm {remoteName} {branchName} disabled={loading} />
		<div class="actions">
			<Button kind="outline" disabled={loading} onclick={() => (selectedBranch[0] = '')}>
				Back
			</Button>
			<Button style="pop" {loading} onclick={setTarget} testId="accept-git-auth">Let's go!</Button>
		</div>
	{:else}
		<ProjectSetupTarget
			projectName={project.title}
			{remoteBranches}
			onBranchSelected={async (branch) => {
				selectedBranch = branch;
				// TODO: Temporary solution to forcing Windows to use system executable
				if (platformName === 'windows') {
					setTarget();
				}
			}}
		/>
	{/if}
</DecorativeSplitView>

<style lang="postcss">
	.actions {
		margin-top: 20px;
		text-align: right;
	}
</style>

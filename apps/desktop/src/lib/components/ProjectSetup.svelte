<script lang="ts">
	import ProjectSetupTarget from './ProjectSetupTarget.svelte';
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import { Project, ProjectService } from '$lib/backend/projects';
	import { BaseBranchService, type RemoteBranchInfo } from '$lib/baseBranch/baseBranchService';
	import DecorativeSplitView from '$lib/components/DecorativeSplitView.svelte';
	import { platformName } from '$lib/platform/platform';
	import KeysForm from '$lib/settings/KeysForm.svelte';
	import { getContext } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';

	interface Props {
		remoteBranches: RemoteBranchInfo[];
	}

	let { remoteBranches }: Props = $props();

	const project = $state(getContext(Project));
	const projectService = getContext(ProjectService);
	const branchController = getContext(BranchController);
	const baseBranchService = getContext(BaseBranchService);

	let selectedBranch = $state(['', '']);
	let loading = $state(false);

	async function setTarget() {
		if (!selectedBranch[0] || selectedBranch[0] === '') return;

		loading = true;
		try {
			// TODO: Refactor temporary solution to forcing Windows to use system executable
			if ($platformName === 'win32') {
				project.preferred_key = 'systemExecutable';
				await projectService.updateProject(project);
				await baseBranchService.refresh();
			}
			await branchController.setTarget(selectedBranch[0], selectedBranch[1]);
			goto(`/${project.id}/`, { invalidateAll: true });
		} finally {
			loading = false;
		}
	}
</script>

<DecorativeSplitView img={newProjectSvg}>
	{#if selectedBranch[0] && selectedBranch[0] !== '' && $platformName !== 'win32'}
		{@const [remoteName, branchName] = selectedBranch[0].split(/\/(.*)/s)}
		<KeysForm {remoteName} {branchName} disabled={loading} />
		<div class="actions">
			<Button style="ghost" outline disabled={loading} onclick={() => (selectedBranch[0] = '')}>
				Back
			</Button>
			<Button style="pop" kind="solid" {loading} onclick={setTarget} testId="accept-git-auth">
				Let's go!
			</Button>
		</div>
	{:else}
		<ProjectSetupTarget
			projectName={project.title}
			{remoteBranches}
			on:branchSelected={async (e) => {
				selectedBranch = e.detail;
				// TODO: Temporary solution to forcing Windows to use system executable
				if ($platformName === 'win32') {
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

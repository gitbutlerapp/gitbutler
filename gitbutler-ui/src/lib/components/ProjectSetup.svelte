<script async lang="ts">
	import Button from './Button.svelte';
	import KeysForm from './KeysForm.svelte';
	import ProjectSetupTarget from './ProjectSetupTarget.svelte';
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import { Project } from '$lib/backend/projects';
	import DecorativeSplitView from '$lib/components/DecorativeSplitView.svelte';
	import { getContext } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { goto } from '$app/navigation';

	export let remoteBranches: { name: string }[];

	const project = getContext(Project);
	const branchController = getContext(BranchController);

	let selectedBranch = '';
	let loading = false;

	async function setTarget() {
		if (!selectedBranch) return;
		loading = true;
		try {
			await branchController.setTarget(selectedBranch);
			goto(`/${project.id}/`);
		} finally {
			loading = false;
		}
	}
</script>

<DecorativeSplitView img={newProjectSvg}>
	{#if selectedBranch}
		{@const [remoteName, branchName] = selectedBranch.split(/\/(.*)/s)}
		<KeysForm {remoteName} {branchName} />
		<div class="actions">
			<Button kind="outlined" color="neutral" on:mousedown={() => (selectedBranch = '')}
				>Back</Button
			>
			<Button color="primary" {loading} on:click={setTarget}>Let's go!</Button>
		</div>
	{:else}
		<ProjectSetupTarget
			projectName={project.title}
			{remoteBranches}
			on:branchSelected={(e) => {
				selectedBranch = e.detail;
			}}
		/>
	{/if}
</DecorativeSplitView>

<style lang="postcss">
	.actions {
		margin-top: var(--size-20);
		text-align: right;
	}
</style>

<script async lang="ts">
	import Button from './Button.svelte';
	import KeysForm from './KeysForm.svelte';
	import ProjectSetupTarget from './ProjectSetupTarget.svelte';
	import DecorativeSplitView from '$lib/components/DecorativeSplitView.svelte';
	import type { AuthService } from '$lib/backend/auth';
	import type { Project, ProjectService } from '$lib/backend/projects';
	import type { UserService } from '$lib/stores/user';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranchService } from '$lib/vbranches/branchStoresCache';
	import { goto } from '$app/navigation';

	export let authService: AuthService;
	export let branchController: BranchController;
	export let baseBranchService: BaseBranchService;
	export let projectService: ProjectService;
	export let project: Project;
	export let userService: UserService;
	export let remoteBranches: { name: string }[];

	$: user$ = userService.user$;

	let selectedBranch = '';
	let loading = false;

	async function setTarget() {
		if (!selectedBranch) return;
		loading = true;
		try {
			await branchController.setTarget(selectedBranch);
			goto('..');
		} finally {
			loading = false;
		}
	}
</script>

<DecorativeSplitView
	user={$user$}
	imgSet={{
		light: '/images/img_moon-door-light.webp',
		dark: '/images/img_moon-door-dark.webp'
	}}
>
	{#if selectedBranch}
		{@const [remoteName, branchName] = selectedBranch.split(/\/(.*)/s)}
		<KeysForm
			{project}
			{authService}
			{baseBranchService}
			{projectService}
			{remoteName}
			{branchName}
		/>
		<div class="actions">
			<Button kind="outlined" on:mousedown={() => (selectedBranch = '')}>Back</Button>
			<Button color="primary" {loading} on:click={setTarget}>Let's go!</Button>
		</div>
	{:else}
		<ProjectSetupTarget
			projectId={project.id}
			{userService}
			{remoteBranches}
			on:branchSelected={(e) => {
				selectedBranch = e.detail;
			}}
		/>
	{/if}
</DecorativeSplitView>

<style lang="postcss">
	.actions {
		margin-top: var(--space-20);
		text-align: right;
	}
</style>

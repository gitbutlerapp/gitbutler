<script lang="ts">
	import { goto } from '$app/navigation';
	import type { Project, ProjectService } from '$lib/backend/projects';
	import type { UserService } from '$lib/stores/user';
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import IconLink from './IconLink.svelte';
	import Select from './Select.svelte';
	import SelectItem from './SelectItem.svelte';
	import Button from './Button.svelte';
	import Link from './Link.svelte';
	import type { BaseBranch } from '$lib/vbranches/types';
	import type { BranchController } from '$lib/vbranches/branchController';
	import { slide } from 'svelte/transition';

	export let projectService: ProjectService;
	export let branchController: BranchController;
	export let project: Project | undefined;
	export let userService: UserService;
	export let baseBranch: BaseBranch;

	const projects$ = projectService.projects$;

	$: user$ = userService.user$;

	let loading = false;
	let showDropDown = false;

	function onSelectItemClick(clickedProject: Project): void {
		goto(`/${clickedProject.id}/`);
	}
</script>

<DecorativeSplitView
	user={$user$}
	imgSet={{
		light: '/images/street-sign-art-light.svg',
		dark: '/images/street-sign-art-dark.svg'
	}}
>
	<div class="switchrepo">
		<p class="switchrepo__title text-base-body-18 text-bold">
			Looks like you've switched away from <span class="repo-name"> gitbutler/integration </span>
		</p>

		<p class="switchrepo__message text-base-body-13">
			Due to GitButler managing multiple virtual branches, you cannot switch back and forth between
			git branches and virtual branches easily.
			<Link href="https://docs.gitbutler.com/features/virtual-branches/integration-branch">
				Learn more
			</Link>
		</p>

		<div class="switchrepo__actions">
			<Button
				color="primary"
				icon="undo-small"
				on:click={() => {
					if (baseBranch) branchController.setTarget(baseBranch.branchName);
				}}
			>
				Go back to gitbutler/integration
			</Button>
			{#if !showDropDown}
				<Button
					color="primary"
					kind="outlined"
					icon="undo-small"
					on:click={() => {
						showDropDown = true;
						// if (baseBranch) branchController.setTarget(baseBranch.branchName);
					}}
				>
					Switch to another project...
				</Button>
			{/if}
		</div>

		{#if showDropDown}
			<div class="switchrepo__project" transition:slide={{ duration: 250 }}>
				<Select
					id="select-project"
					label="Switch to another project"
					itemId="id"
					labelId="title"
					items={$projects$}
					value={project}
					on:select={(e) => onSelectItemClick(e.detail.item)}
				>
					<SelectItem slot="template" let:item selected={item.id == project?.id}>
						{item.title}
					</SelectItem>
					<SelectItem
						slot="append"
						icon="plus"
						{loading}
						on:click={async () => {
							loading = true;
							try {
								await projectService.addProject();
							} finally {
								loading = false;
							}
						}}
					>
						Add new project
					</SelectItem>
				</Select>
				<div class="switchrepo__change">
					<Button icon="chevron-right-small">Open project</Button>
				</div>
			</div>
		{/if}
	</div>
	<svelte:fragment slot="links">
		<IconLink icon="docs" href="https://docs.gitbutler.com/features/virtual-branches/branch-lanes">
			GitButler Docs
		</IconLink>
		<IconLink icon="video" href="https://www.youtube.com/@gitbutlerapp">Watch tutorial</IconLink>
	</svelte:fragment>
</DecorativeSplitView>

<style lang="postcss">
	.switchrepo {
		max-width: 36rem;
	}

	.switchrepo__title {
		color: var(--clr-theme-scale-ntrl-30);
		margin-bottom: var(--space-12);
	}

	.switchrepo__message {
		color: var(--clr-theme-scale-ntrl-50);
		margin-bottom: var(--space-20);
	}
	.switchrepo__actions {
		display: flex;
		gap: var(--space-6);
		padding-bottom: var(--space-24);
	}

	.switchrepo__project {
		padding-top: var(--space-24);
		border-top: 1px dashed var(--clr-theme-scale-ntrl-60);
	}

	.switchrepo__change {
		text-align: right;
		margin-top: var(--space-10);
		padding-bottom: var(--space-32);
	}

	.repo-name {
		font-family: Iosevka, monospace;
		border-radius: var(--radius-s);
		background: var(--clr-theme-container-sub);
		padding: var(--space-2) var(--space-4);
	}
</style>

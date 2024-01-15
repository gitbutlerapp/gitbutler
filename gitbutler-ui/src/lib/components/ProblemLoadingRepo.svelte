<script lang="ts">
	import { goto } from '$app/navigation';
	import type { Project, ProjectService } from '$lib/backend/projects';
	import type { UserService } from '$lib/stores/user';
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import Icon from '$lib/icons/Icon.svelte';
	import IconLink from './IconLink.svelte';
	import Select from './Select.svelte';
	import SelectItem from './SelectItem.svelte';
	import Button from './Button.svelte';

	export let projectService: ProjectService;
	export let project: Project | undefined;
	export let userService: UserService;
	export let error: any = undefined;

	const projects$ = projectService.projects$;

	$: user$ = userService.user$;

	let loading = false;

	function onSelectItemClick(clickedProject: Project): void {
		goto(`/${clickedProject.id}/`);
	}
</script>

<DecorativeSplitView user={$user$}>
	<div class="problem">
		<p class="problem__title text-base-body-18 text-bold">There was a problem loading this repo</p>

		{#if error}
			<div class="problem__error text-base-body-12">
				<Icon name="error" color="error" />
				{error}
			</div>
		{/if}

		<div class="problem__project">
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
			<div class="problem__change">
				<Button icon="chevron-right-small">Open project</Button>
			</div>
			<div class="problem__delete">
				<Button wide kind="outlined" color="error" icon="bin-small">
					Remove project from GitButler
				</Button>
			</div>
		</div>
	</div>
	<svelte:fragment slot="links">
		<IconLink icon="docs" href="https://docs.gitbutler.com/features/virtual-branches/branch-lanes">
			GitButler Docs
		</IconLink>
		<IconLink icon="video" href="https://www.youtube.com/@gitbutlerapp">Watch tutorial</IconLink>
	</svelte:fragment>
</DecorativeSplitView>

<style lang="postcss">
	.problem {
	}

	.problem__title {
		color: var(--clr-theme-scale-ntrl-30);
		margin-bottom: var(--space-12);
	}

	.problem__change {
		text-align: right;
		margin-top: var(--space-10);
		padding-bottom: var(--space-32);
		border-bottom: 1px dashed var(--clr-theme-scale-ntrl-60);
	}

	.problem__delete {
		margin-top: var(--space-32);
	}

	.problem__error {
		display: flex;
		color: var(--clr-theme-scale-ntrl-0);
		gap: var(--space-12);
		padding: var(--space-20);
		background-color: var(--clr-theme-err-container);
		border-radius: var(--radius-m);
		margin-bottom: var(--space-24);
	}
</style>

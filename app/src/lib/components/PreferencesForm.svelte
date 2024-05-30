<script lang="ts">
	import SectionCard from './SectionCard.svelte';
	import Section from '$lib/components/settings/Section.svelte';
	import Spacer from './Spacer.svelte';
	import TextBox from './TextBox.svelte';
	import { Project, ProjectService } from '$lib/backend/projects';
	import Toggle from '$lib/components/Toggle.svelte';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { getContext } from '$lib/utils/context';

	const projectService = getContext(ProjectService);
	const project = getContext(Project);

	let snaphotLinesThreshold = project?.snapshot_lines_threshold || 20; // when undefined, the default is 20
	let allowForcePushing = project?.ok_with_force_push;
	let omitCertificateCheck = project?.omit_certificate_check;
	let patchStackBranches = project?.patch_stack_branches;

	const runCommitHooks = projectRunCommitHooks(project.id);

	async function setWithForcePush(value: boolean) {
		project.ok_with_force_push = value;
		await projectService.updateProject(project);
	}

	async function setOmitCertificateCheck(value: boolean | undefined) {
		project.omit_certificate_check = !!value;
		await projectService.updateProject(project);
	}

	async function setSnapshotLinesThreshold(value: number) {
		project.snapshot_lines_threshold = value;
		await projectService.updateProject(project);
	}

	async function setPatchStackBranches(value: boolean) {
		project.patch_stack_branches = value;
		await projectService.updateProject(project);
	}
</script>

<Section>
	<svelte:fragment slot="title">Preferences</svelte:fragment>
	<svelte:fragment slot="description">
		Other settings to customize your GitButler experience.
	</svelte:fragment>
	<SectionCard orientation="row" labelFor="allowForcePush">
		<svelte:fragment slot="title">Allow force pushing</svelte:fragment>
		<svelte:fragment slot="caption">
			Force pushing allows GitButler to override branches even if they were pushed to remote.
			We will never force push to the trunk.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="allowForcePush"
				bind:checked={allowForcePushing}
				on:change={async () => await setWithForcePush(allowForcePushing)}
			/>
		</svelte:fragment>
	</SectionCard>

	<SectionCard orientation="row" labelFor="omitCertificateCheck">
		<svelte:fragment slot="title">Ignore host certificate checks</svelte:fragment>
		<svelte:fragment slot="caption">
			Enabling this will ignore host certificate checks when authenticating with ssh.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="omitCertificateCheck"
				bind:checked={omitCertificateCheck}
				on:change={async () => await setOmitCertificateCheck(omitCertificateCheck)}
			/>
		</svelte:fragment>
	</SectionCard>

	<SectionCard labelFor="runHooks" orientation="row">
		<svelte:fragment slot="title">Run commit hooks</svelte:fragment>
		<svelte:fragment slot="caption">
			Enabling this will run any git pre and post commit hooks you have configured in your
			repository.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle id="runHooks" bind:checked={$runCommitHooks} />
		</svelte:fragment>
	</SectionCard>

	<SectionCard orientation="row" centerAlign>
		<svelte:fragment slot="title">Snapshot lines threshold</svelte:fragment>
		<svelte:fragment slot="caption">
			The number of lines that trigger a snapshot when saving.
		</svelte:fragment>

		<svelte:fragment slot="actions">
			<TextBox
				type="number"
				width={100}
				textAlign="center"
				value={snaphotLinesThreshold?.toString()}
				minVal={5}
				maxVal={1000}
				showCountActions
				on:change={(e) => {
					setSnapshotLinesThreshold(parseInt(e.detail));
				}}
			/>
		</svelte:fragment>
	</SectionCard>
</Section>

<Spacer />

<Section>
	<svelte:fragment slot="title">Experimental Stuff</svelte:fragment>
	<svelte:fragment slot="description">
		Configure the authentication flow for GitButler when authenticating with your Git remote
		provider.
	</svelte:fragment>
	<SectionCard orientation="row" labelFor="patchStackBranches">
		<svelte:fragment slot="title">Patch Stack Branches</svelte:fragment>
		<svelte:fragment slot="caption">
			This mode enables "patch stack branches" which allows for a more flexible way of
			managing commits in a branch. It enables much easier rebase-style patch-focused
			workflows. It is highly experimental and not recommended for general use.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="patchStackBranches"
				bind:checked={patchStackBranches}
				on:change={async () => await setPatchStackBranches(patchStackBranches)}
			/>
		</svelte:fragment>
	</SectionCard>
</Section>

<Spacer />

<style lang="post-css">
	.wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
	}
</style>

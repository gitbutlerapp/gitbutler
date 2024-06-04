<script lang="ts">
	import SectionCard from './SectionCard.svelte';
	import Spacer from './Spacer.svelte';
	import TextBox from './TextBox.svelte';
	import { GitConfigService } from '$lib/backend/gitConfigService';
	import { Project, ProjectService } from '$lib/backend/projects';
	import Toggle from '$lib/components/Toggle.svelte';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { getContext } from '$lib/utils/context';
	import { onMount } from 'svelte';

	const projectService = getContext(ProjectService);
	const project = getContext(Project);

	let snaphotLinesThreshold = project?.snapshot_lines_threshold || 20; // when undefined, the default is 20
	let allowForcePushing = project?.ok_with_force_push;
	let omitCertificateCheck = project?.omit_certificate_check;

	const gitConfig = getContext(GitConfigService);
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

	let signCommits = false;
	async function setSignCommits(value: boolean) {
		signCommits = value;
		await gitConfig.setGbConfig(project.id, { signCommits: value });
	}

	onMount(async () => {
		signCommits = (await gitConfig.getGbConfig(project.id)).signCommits || false;
	});
</script>

<section class="wrapper">
	<SectionCard orientation="row" labelFor="allowForcePush">
		<svelte:fragment slot="title">Allow force pushing</svelte:fragment>
		<svelte:fragment slot="caption">
			Force pushing allows GitButler to override branches even if they were pushed to remote. We
			will never force push to the trunk.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="allowForcePush"
				bind:checked={allowForcePushing}
				on:change={async () => await setWithForcePush(allowForcePushing)}
			/>
		</svelte:fragment>
	</SectionCard>

	<SectionCard orientation="row" labelFor="allowForcePush">
		<svelte:fragment slot="title">Sign commits</svelte:fragment>
		<svelte:fragment slot="caption">
			GitButler will sign commits as per your git configuration.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="signCommits"
				bind:checked={signCommits}
				on:change={async () => await setSignCommits(signCommits)}
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
</section>

<Spacer />

<style lang="post-css">
	.wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
	}
</style>

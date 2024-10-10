<script lang="ts">
	import { Project, ProjectService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { projectRunCommitHooks } from '$lib/config/config';
	import Section from '$lib/settings/Section.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { getContext } from '@gitbutler/shared/context';

	const projectService = getContext(ProjectService);
	const project = getContext(Project);

	let snaphotLinesThreshold = project?.snapshot_lines_threshold || 20; // when undefined, the default is 20
	let allowForcePushing = project?.ok_with_force_push;
	let omitCertificateCheck = project?.omit_certificate_check;
	let useNewLocking = project?.use_new_locking || false;

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

	async function setUseNewLocking(value: boolean) {
		project.use_new_locking = value;
		await projectService.updateProject(project);
	}

	$: setUseNewLocking(useNewLocking);

	async function handleAllowForcePushClick(event: MouseEvent) {
		await setWithForcePush((event.target as HTMLInputElement)?.checked);
	}

	async function handleOmitCertificateCheckClick(event: MouseEvent) {
		await setOmitCertificateCheck((event.target as HTMLInputElement)?.checked);
	}
</script>

<Section gap={8}>
	<SectionCard orientation="row" labelFor="allowForcePush">
		<svelte:fragment slot="title">Allow force pushing</svelte:fragment>
		<svelte:fragment slot="caption">
			Force pushing allows GitButler to override branches even if they were pushed to remote.
			GitButler will never force push to the target branch.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="allowForcePush"
				checked={allowForcePushing}
				on:click={handleAllowForcePushClick}
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
				checked={omitCertificateCheck}
				on:click={handleOmitCertificateCheckClick}
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

	<SectionCard labelFor="useNewLocking" orientation="row">
		<svelte:fragment slot="title">Use new experimental hunk locking algorithm</svelte:fragment>
		<svelte:fragment slot="caption">
			This new hunk locking algorithm is still in the testing phase but should more accurately catch
			locks and subsequently cause fewer errors.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle id="useNewLocking" bind:checked={useNewLocking} />
		</svelte:fragment>
	</SectionCard>
</Section>

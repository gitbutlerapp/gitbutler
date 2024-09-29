<script lang="ts">
	import { Project, ProjectService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { projectRunCommitHooks } from '$lib/config/config';
	import Section from '$lib/settings/Section.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { getContext } from '$lib/utils/context';
	import { run } from 'svelte/legacy';

	const projectService = getContext(ProjectService);
	const project = $state(getContext(Project));

	let snaphotLinesThreshold = project?.snapshot_lines_threshold || 20; // when undefined, the default is 20
	let allowForcePushing = project?.ok_with_force_push;
	let omitCertificateCheck = project?.omit_certificate_check;
	let useNewLocking = $state(project?.use_new_locking || false);

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

	let succeedingRebases = $state(project.succeedingRebases);

	run(() => {
		project.succeedingRebases = succeedingRebases;
		projectService.updateProject(project);
	});

	async function setUseNewLocking(value: boolean) {
		project.use_new_locking = value;
		await projectService.updateProject(project);
	}

	run(() => {
		setUseNewLocking(useNewLocking);
	});

	async function handleAllowForcePushClick(event: MouseEvent) {
		await setWithForcePush((event.target as HTMLInputElement)?.checked);
	}

	async function handleOmitCertificateCheckClick(event: MouseEvent) {
		await setOmitCertificateCheck((event.target as HTMLInputElement)?.checked);
	}
</script>

<Section gap={8}>
	<SectionCard orientation="row" labelFor="allowForcePush">
		{#snippet title()}
			Allow force pushing
		{/snippet}
		{#snippet caption()}
			Force pushing allows GitButler to override branches even if they were pushed to remote.
			GitButler will never force push to the target branch.
		{/snippet}
		{#snippet actions()}
			<Toggle id="allowForcePush" checked={allowForcePushing} onclick={handleAllowForcePushClick} />
		{/snippet}
	</SectionCard>

	<SectionCard orientation="row" labelFor="omitCertificateCheck">
		{#snippet title()}
			Ignore host certificate checks
		{/snippet}
		{#snippet caption()}
			Enabling this will ignore host certificate checks when authenticating with ssh.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="omitCertificateCheck"
				checked={omitCertificateCheck}
				onclick={handleOmitCertificateCheckClick}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard labelFor="runHooks" orientation="row">
		{#snippet title()}
			Run commit hooks
		{/snippet}
		{#snippet caption()}
			Enabling this will run any git pre and post commit hooks you have configured in your
			repository.
		{/snippet}
		{#snippet actions()}
			<Toggle id="runHooks" bind:checked={$runCommitHooks} />
		{/snippet}
	</SectionCard>

	<SectionCard orientation="row" centerAlign>
		{#snippet title()}
			Snapshot lines threshold
		{/snippet}
		{#snippet caption()}
			The number of lines that trigger a snapshot when saving.
		{/snippet}

		{#snippet actions()}
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
		{/snippet}
	</SectionCard>

	<SectionCard labelFor="useNewLocking" orientation="row">
		{#snippet title()}
			Use new experimental hunk locking algorithm
		{/snippet}
		{#snippet caption()}
			This new hunk locking algorithm is still in the testing phase but should more accurately catch
			locks and subsequently cause fewer errors.
		{/snippet}
		{#snippet actions()}
			<Toggle id="useNewLocking" bind:checked={useNewLocking} />
		{/snippet}
	</SectionCard>

	<SectionCard labelFor="succeedingRebases" orientation="row">
		{#snippet title()}
			Edit mode and succeeding rebases
		{/snippet}
		{#snippet caption()}
			This is an experimental setting which will ensure that rebasing will always succeed,
			introduces a mode for editing individual commits, and adds the ability to resolve conflicted
			commits.
		{/snippet}
		{#snippet actions()}
			<Toggle id="succeedingRebases" bind:checked={succeedingRebases} />
		{/snippet}
	</SectionCard>
</Section>

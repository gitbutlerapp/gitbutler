<script lang="ts">
	import Checkbox from '$lib/components/Checkbox.svelte';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { createEventDispatcher } from 'svelte';
	import type { Project } from '$lib/backend/projects';

	export let project: Project;

	let allowForcePushing = project?.ok_with_force_push;
	let omitCertificateCheck = project?.omit_certificate_check;

	const runCommitHooks = projectRunCommitHooks(project.id);
	const dispatch = createEventDispatcher<{
		updated: {
			ok_with_force_push?: boolean;
			omit_certificate_check?: boolean;
		};
	}>();
</script>

<div class="flex flex-1 flex-col gap-1 rounded-lg border border-light-400 p-2 dark:border-dark-500">
	<form class="flex items-center gap-1">
		<Checkbox
			name="allow-force-pushing"
			checked={allowForcePushing}
			on:change={() => {
				allowForcePushing = !allowForcePushing;
				dispatch('updated', { ok_with_force_push: allowForcePushing });
			}}
		/>
		<label class="ml-2" for="allow-force-pushing">
			<div>Allow force pushing</div>
		</label>
	</form>
	<p class="ml-7 text-light-700 dark:text-dark-200">
		Force pushing allows GitButler to override branches even if they were pushed to remote. We will
		never force push to the trunk.
	</p>

	<form class="flex items-center gap-1">
		<Checkbox
			name="omit-certificate-check"
			checked={omitCertificateCheck}
			on:change={() => {
				omitCertificateCheck = !omitCertificateCheck;
				dispatch('updated', { omit_certificate_check: omitCertificateCheck });
			}}
		/>
		<label class="ml-2" for="allow-force-pushing">
			<div>Ignore host certificate checks</div>
		</label>
	</form>
	<p class="ml-7 text-light-700 dark:text-dark-200">
		Enabling this will ignore host certificate checks when authenticating with ssh.
	</p>

	<form class="flex items-center gap-1">
		<Checkbox
			name="run-commit-hooks"
			checked={$runCommitHooks}
			on:change={() => {
				$runCommitHooks = !$runCommitHooks;
			}}
		/>
		<label class="ml-2" for="allow-force-pushing">
			<div>Run commit hooks</div>
		</label>
	</form>
	<p class="ml-7 text-light-700 dark:text-dark-200">
		Enabling this will run any git pre and post commit hooks you have configured in your repository.
	</p>
</div>

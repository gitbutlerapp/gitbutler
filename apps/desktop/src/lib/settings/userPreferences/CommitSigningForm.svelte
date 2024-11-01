<script lang="ts">
	import { run } from 'svelte/legacy';

	import { GitConfigService } from '$lib/backend/gitConfigService';
	import { Project, ProjectsService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import SectionCardDisclaimer from '$lib/components/SectionCardDisclaimer.svelte';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import Section from '$lib/settings/Section.svelte';
	import InfoMessage from '$lib/shared/InfoMessage.svelte';
	import Link from '$lib/shared/Link.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';

	const projectsService = getContext(ProjectsService);
	const project = getContext(Project);

	let useNewLocking = project?.use_experimental_locking || false;
	let signCommits = $state(false);

	const gitConfig = getContext(GitConfigService);

	async function setSignCommits(targetState: boolean) {
		signCommits = targetState;
		await gitConfig.setGbConfig(project.id, { signCommits: targetState });
	}

	// gpg.format
	let signingFormat = $state('openpgp');
	// user.signingkey
	let signingKey = $state('');
	// gpg.ssh.program / gpg.program
	let signingProgram = $state('');

	const signingFormatOptions = [
		{
			label: 'GPG',
			value: 'openpgp'
		},
		{
			label: 'SSH',
			value: 'ssh'
		}
	];

	let checked = $state(false);
	let loading = $state(true);
	let signCheckResult = $state(false);
	let errorMessage = $state('');

	async function checkSigning() {
		checked = true;
		loading = true;
		await invoke('check_signing_settings', { id: project.id })
			.then((_) => {
				signCheckResult = true;
			})
			.catch((err) => {
				console.error('Error checking signing:', err);
				console.log(err.message);
				errorMessage = err.message;
				signCheckResult = false;
			});
		loading = false;
	}

	async function updateSigningInfo() {
		let signUpdate = {
			signingFormat: signingFormat,
			signingKey: signingKey,
			gpgProgram: signingFormat === 'openpgp' ? signingProgram : '',
			gpgSshProgram: signingFormat === 'ssh' ? signingProgram : ''
		};
		await gitConfig.setGbConfig(project.id, signUpdate);
	}

	async function setUseNewLocking(value: boolean) {
		project.use_experimental_locking = value;
		await projectsService.updateProject(project);
	}

	run(() => {
		setUseNewLocking(useNewLocking);
	});

	onMount(async () => {
		let gitConfigSettings = await gitConfig.getGbConfig(project.id);
		signCommits = gitConfigSettings.signCommits || false;
		signingFormat = gitConfigSettings.signingFormat || 'openpgp';
		signingKey = gitConfigSettings.signingKey || '';
		if (signingFormat === 'openpgp') {
			signingProgram = gitConfigSettings.gpgProgram || '';
		} else {
			signingProgram = gitConfigSettings.gpgSshProgram || '';
		}
	});

	async function handleSignCommitsClick(event: MouseEvent) {
		await setSignCommits((event.target as HTMLInputElement)?.checked);
	}
</script>

<Section>
	<SectionCard orientation="row" labelFor="signCommits">
		<svelte:fragment slot="title">Sign commits</svelte:fragment>
		<svelte:fragment slot="caption">
			Use GPG or SSH to sign your commits so they can be verified as authentic.
			<br />
			GitButler will sign commits as per your git configuration.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle id="signCommits" checked={signCommits} onclick={handleSignCommitsClick} />
		</svelte:fragment>
	</SectionCard>
	{#if signCommits}
		<SectionCard orientation="column">
			<Select
				value={signingFormat}
				options={signingFormatOptions}
				label="Signing format"
				onselect={(value: string) => {
					signingFormat = value;
					updateSigningInfo();
				}}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === signingFormat} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>

			<Textbox
				label="Signing key"
				bind:value={signingKey}
				required
				onchange={updateSigningInfo}
				placeholder="ex: /Users/bob/.ssh/id_rsa.pub"
			/>

			<Textbox
				label="Signing program (optional)"
				bind:value={signingProgram}
				onchange={updateSigningInfo}
				placeholder="ex: /Applications/1Password.app/Contents/MacOS/op-ssh-sign"
			/>

			{#if checked}
				<InfoMessage
					style={loading ? 'neutral' : signCheckResult ? 'success' : 'error'}
					filled
					outlined={false}
				>
					<svelte:fragment slot="title">
						{#if loading}
							<p>Checking signing</p>
						{:else if signCheckResult}
							<p>Signing is working correctly</p>
						{:else}
							<p>Signing is not working correctly</p>
							<pre>{errorMessage}</pre>
						{/if}
					</svelte:fragment>
				</InfoMessage>
			{/if}

			<Button style="pop" kind="solid" wide icon="item-tick" onclick={checkSigning}>
				{#if !checked}
					Test signing
				{:else}
					Re-test signing
				{/if}
			</Button>
			<SectionCardDisclaimer>
				Signing commits can allow other people to verify your commits if you publish the public
				version of your signing key.
				<Link href="https://docs.gitbutler.com/features/virtual-branches/verifying-commits"
					>Read more</Link
				> about commit signing and verification.
			</SectionCardDisclaimer>
		</SectionCard>
	{/if}
</Section>

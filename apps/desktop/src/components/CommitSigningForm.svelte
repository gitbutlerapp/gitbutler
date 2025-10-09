<script lang="ts">
	import InfoMessage from '$components/InfoMessage.svelte';
	import Section from '$components/Section.svelte';
	import SectionCardDisclaimer from '$components/SectionCardDisclaimer.svelte';
	import { GIT_CONFIG_SERVICE } from '$lib/config/gitConfigService';
	import { GIT_SERVICE } from '$lib/git/gitService';
	import { inject } from '@gitbutler/core/context';
	import { Button, Link, SectionCard, Select, SelectItem, Textbox, Toggle } from '@gitbutler/ui';

	import { onMount } from 'svelte';

	const { projectId }: { projectId: string } = $props();

	const gitConfig = inject(GIT_CONFIG_SERVICE);
	const gitService = inject(GIT_SERVICE);

	let signCommits = $state(false);

	async function setSignCommits(targetState: boolean) {
		signCommits = targetState;
		await gitConfig.setGbConfig(projectId, { signCommits: targetState });
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
			value: 'openpgp',
			keyPlaceholder: 'ex: 723CCA3AC13CF28D',
			programPlaceholder: 'ex: /usr/local/bin/gpg'
		},
		{
			label: 'SSH',
			value: 'ssh',
			keyPlaceholder: 'ex: /Users/bob/.ssh/id_rsa.pub',
			programPlaceholder: 'ex: /Applications/1Password.app/Contents/MacOS/op-ssh-sign'
		}
	] as const;

	const selectedOption = $derived(
		signingFormatOptions.find((option) => option.value === signingFormat)
	);
	const keyPlaceholder = $derived(selectedOption?.keyPlaceholder);
	const programPlaceholder = $derived(selectedOption?.programPlaceholder);

	let checked = $state(false);
	let loading = $state(true);
	let signCheckResult = $state(false);
	let errorMessage = $state('');

	async function checkSigning() {
		errorMessage = '';
		checked = true;
		loading = true;
		await gitService
			.checkSigningSettings(projectId)
			.then(() => {
				signCheckResult = true;
			})
			.catch((err) => {
				console.error('Error checking signing:', err);
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
		await gitConfig.setGbConfig(projectId, signUpdate);
	}

	onMount(async () => {
		let gitConfigSettings = await gitConfig.getGbConfig(projectId);
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
		{#snippet title()}
			Sign commits
		{/snippet}
		{#snippet caption()}
			Use GPG or SSH to sign your commits so they can be verified as authentic.
			<br />
			GitButler will sign commits as per your git configuration, but evaluates
			<code class="code-string">gitbutler.signCommits</code> with priority.
		{/snippet}
		{#snippet actions()}
			<Toggle id="signCommits" checked={signCommits} onclick={handleSignCommitsClick} />
		{/snippet}
	</SectionCard>
	{#if signCommits}
		<SectionCard orientation="column">
			<Select
				value={signingFormat}
				options={signingFormatOptions}
				wide
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
				placeholder={keyPlaceholder}
			/>

			<Textbox
				label="Signing program (optional)"
				bind:value={signingProgram}
				onchange={updateSigningInfo}
				placeholder={programPlaceholder}
			/>

			{#if checked}
				<InfoMessage
					style={loading ? 'info' : signCheckResult ? 'success' : 'error'}
					filled
					outlined={false}
				>
					{#snippet title()}
						{#if loading}
							<p>Checking signing</p>
						{:else if signCheckResult}
							<p>Signing is working correctly</p>
						{:else}
							<p>Signing is not working correctly</p>
						{/if}
					{/snippet}

					{#snippet content()}
						{#if errorMessage}
							<pre>{errorMessage}</pre>
						{/if}
					{/snippet}
				</InfoMessage>
			{/if}

			<Button style="pop" wide icon="item-tick" onclick={checkSigning}>
				{#if !checked}
					Test signing
				{:else}
					Re-test signing
				{/if}
			</Button>
			<SectionCardDisclaimer>
				Signing commits can allow other people to verify your commits if you publish the public
				version of your signing key.
				<Link href="https://docs.gitbutler.com/features/virtual-branches/signing-commits"
					>Read more</Link
				> about commit signing and verification.
			</SectionCardDisclaimer>
		</SectionCard>
	{/if}
</Section>

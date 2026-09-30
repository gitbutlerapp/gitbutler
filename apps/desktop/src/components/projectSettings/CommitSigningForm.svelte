<script lang="ts">
	import SectionCardDisclaimer from "$components/shared/SectionCardDisclaimer.svelte";
	import SettingsSection from "$components/shared/SettingsSection.svelte";
	import { GIT_CONFIG_SERVICE } from "$lib/config/gitConfigService";
	import { GIT_SERVICE } from "$lib/git/gitService";
	import { t } from "$lib/i18n";
	import { inject } from "@gitbutler/core/context";
	import {
		Button,
		CardGroup,
		InfoMessage,
		Link,
		Select,
		SelectItem,
		Textbox,
		Toggle,
	} from "@gitbutler/ui-svelte";

	const { projectId }: { projectId: string } = $props();

	const gitConfig = inject(GIT_CONFIG_SERVICE);
	const gitService = inject(GIT_SERVICE);

	async function setSignCommits(targetState: boolean) {
		signCommits = targetState;
		await gitConfig.setGbConfig(projectId, { signCommits: targetState });
	}

	const signingFormatOptions = [
		{
			label: "GPG",
			value: "openpgp",
			keyPlaceholder: "ex: 723CCA3AC13CF28D",
			programPlaceholder: "ex: /usr/local/bin/gpg",
		},
		{
			label: "SSH",
			value: "ssh",
			keyPlaceholder: "ex: /Users/bob/.ssh/id_rsa.pub",
			programPlaceholder: "ex: /Applications/1Password.app/Contents/MacOS/op-ssh-sign",
		},
	] as const;

	const selectedOption = $derived(
		signingFormatOptions.find((option) => option.value === signingFormat),
	);
	const keyPlaceholder = $derived(selectedOption?.keyPlaceholder);
	const programPlaceholder = $derived(selectedOption?.programPlaceholder);

	let checked = $state(false);
	let loading = $state(true);
	let signCheckResult = $state(false);
	let errorMessage = $state("");

	async function checkSigning() {
		errorMessage = "";
		checked = true;
		loading = true;
		await gitService
			.checkSigningSettings(projectId)
			.then(() => {
				signCheckResult = true;
			})
			.catch((err) => {
				console.error("Error checking signing:", err);
				errorMessage = err.message;
				signCheckResult = false;
			});
		loading = false;
	}

	async function updateSigningInfo() {
		let signUpdate = {
			signingFormat: signingFormat,
			signingKey: signingKey,
			gpgProgram: signingFormat === "openpgp" ? signingProgram : "",
			gpgSshProgram: signingFormat === "ssh" ? signingProgram : "",
		};
		await gitConfig.setGbConfig(projectId, signUpdate);
	}

	const gbConfig = $derived(gitConfig.gbConfig(projectId));
	let signCommits = $derived(gbConfig.response?.signCommits ?? false);
	let signingFormat = $derived(gbConfig.response?.signingFormat ?? "openpgp");
	let signingKey = $derived(gbConfig.response?.signingKey ?? "");
	let signingProgram = $derived(
		gbConfig.response
			? signingFormat === "openpgp"
				? (gbConfig.response.gpgProgram ?? "")
				: (gbConfig.response.gpgSshProgram ?? "")
			: "",
	);

	async function handleSignCommitsClick(event: MouseEvent) {
		await setSignCommits((event.target as HTMLInputElement)?.checked);
	}
</script>

<SettingsSection>
	<CardGroup>
		<CardGroup.Item labelFor="signCommits">
			{#snippet title()}
				{t("sign-commits")}
			{/snippet}
			{#snippet caption()}
				{t("use-gpg-or-ssh-to-sign-your-commits-so-they-can-be-verified-")}
				<br />
				{t("gitbutler-will-sign-commits-as-per-your-git-configuration-bu")}
				<code class="code-string">gitbutler.signCommits</code>
				{t("with-priority")}
			{/snippet}
			{#snippet actions()}
				<Toggle id="signCommits" checked={signCommits} onclick={handleSignCommitsClick} />
			{/snippet}
		</CardGroup.Item>
	</CardGroup>
	{#if signCommits}
		<CardGroup>
			<CardGroup.Item>
				<Select
					value={signingFormat}
					options={signingFormatOptions}
					wide
					label={t("signing-format")}
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
					label={t("signing-key")}
					bind:value={signingKey}
					required
					onchange={updateSigningInfo}
					placeholder={keyPlaceholder}
				/>

				<Textbox
					label={t("signing-program-optional")}
					bind:value={signingProgram}
					onchange={updateSigningInfo}
					placeholder={programPlaceholder}
				/>

				{#if checked}
					<InfoMessage
						style={loading ? "info" : signCheckResult ? "success" : "danger"}
						filled
						outlined={false}
						error={errorMessage}
					>
						{#snippet title()}
							{#if loading}
								<p>{t("checking-signing")}</p>
							{:else if signCheckResult}
								<p>{t("signing-is-working-correctly")}</p>
							{:else}
								<p>{t("signing-is-not-working-correctly")}</p>
							{/if}
						{/snippet}
					</InfoMessage>
				{/if}

				<Button style="pop" wide icon="tick" onclick={checkSigning}>
					{#if !checked}
						{t("test-signing")}
					{:else}
						{t("re-test-signing")}
					{/if}
				</Button>
				<SectionCardDisclaimer>
					{t("signing-commits-can-allow-other-people-to-verify-your-commit")}
					<Link href="https://docs.gitbutler.com/features/virtual-branches/signing-commits"
						>{t("read-more")}</Link
					>
					{t("about-commit-signing-and-verification")}
				</SectionCardDisclaimer>
			</CardGroup.Item>
		</CardGroup>
	{/if}
</SettingsSection>

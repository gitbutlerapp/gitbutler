<script lang="ts">
	import CliSymlinkSetup from "$components/settings/CliSymlinkSetup.svelte";
	import { BACKEND } from "$lib/backend";
	import { getUserErrorCode } from "$lib/backend/ipc";
	import { CLI_MANAGER } from "$lib/config/cli";
	import { showToast } from "$lib/notifications/toasts";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { inject } from "@gitbutler/core/context";
	import { Button, CardGroup } from "@gitbutler/ui";

	const backend = inject(BACKEND);
	const platformName = backend.platformName;
	const settingsService = inject(SETTINGS_SERVICE);
	const appSettings = settingsService.appSettings;

	const cliManager = inject(CLI_MANAGER);
	const [installCli, installing] = cliManager.install;
	const [uninstallCli, uninstalling] = cliManager.uninstall;
	const cliState = cliManager.state();

	let showSymlink = $state(false);

	const managedByPackageManager = $derived(!!$appSettings?.ui.cliIsManagedByPackageManager);
	const linkState = $derived(cliState.response);

	async function install() {
		try {
			await installCli();
		} catch (err: unknown) {
			// osascript returns a generic non-success when the user dismisses the
			// macOS admin-privileges prompt. The backend tags that specific case
			// so we can show an info toast instead of an error toast.
			if (getUserErrorCode(err) === "CliInstallCancelled") {
				showToast({ style: "info", message: "CLI install cancelled." });
				return;
			}
			throw err;
		}
	}

	async function uninstall() {
		try {
			await uninstallCli();
		} catch (err: unknown) {
			if (getUserErrorCode(err) === "CliUninstallCancelled") {
				showToast({ style: "info", message: "CLI uninstall cancelled." });
				return;
			}
			throw err;
		}
	}
</script>

<CardGroup>
	<CardGroup.Item>
		{#snippet title()}
			GitButler CLI <code class="code-string">but</code>
		{/snippet}

		{#snippet caption()}
			{#if managedByPackageManager}
				The <code>but</code> CLI is managed by your package manager. Please use your package manager to
				install, update, or remove it.
			{:else if linkState?.blockedReason}
				{linkState.blockedReason}
			{:else if linkState?.installed}
				Installed at <code class="code-string">{linkState.linkPath}</code>. Removing it deletes the
				symlink only — the <code>but</code> binary inside GitButler stays where it is.
			{:else if platformName === "windows"}
				On Windows, you can manually copy the executable (<code>`but`</code>) to a directory in your
				PATH. Click "Show command" for instructions.
			{:else}
				Installs the GitButler CLI (<code>`but`</code>) in your PATH, allowing you to use it from
				the terminal. This action will request admin privileges. Alternatively, you could create a
				symlink manually.
			{/if}
		{/snippet}

		{#if !managedByPackageManager}
			<div class="flex flex-col gap-16">
				<div class="flex gap-8 justify-end">
					{#if linkState?.installed}
						<Button
							style="danger"
							kind="outline"
							icon="bin"
							loading={uninstalling.current.isLoading}
							onclick={uninstall}>Uninstall</Button
						>
					{:else if platformName !== "windows"}
						<Button style="pop" icon="play" loading={installing.current.isLoading} onclick={install}
							>Install But CLI</Button
						>
					{/if}
					<Button
						style="gray"
						kind="outline"
						disabled={showSymlink}
						onclick={() => (showSymlink = !showSymlink)}>Show command</Button
					>
				</div>
			</div>

			{#if showSymlink}
				<CliSymlinkSetup class="m-t-14" />
			{/if}
		{/if}
	</CardGroup.Item>
</CardGroup>

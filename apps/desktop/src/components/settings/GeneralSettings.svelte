<script lang="ts">
	import { goto } from "$app/navigation";
	import CliSymlinkSetup from "$components/settings/CliSymlinkSetup.svelte";
	import AccessTokenSignIn from "$components/shared/AccessTokenSignIn.svelte";
	import { BACKEND } from "$lib/backend";
	import { getUserErrorCode } from "$lib/backend/ipc";
	import { CLI_MANAGER } from "$lib/config/cli";
	import { APP_LOCALES, type AppLocale } from "$lib/i18n";
	import { t } from "$lib/i18n";
	import { showToast } from "$lib/notifications/toasts";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { TERMINAL_SERVICE } from "$lib/settings/terminalService";
	import {
		UI_STATE,
		type CodeEditorSettings,
		type TerminalSettings,
	} from "$lib/state/uiState.svelte";
	import { UPDATER_SERVICE } from "$lib/updater/updater";
	import { USER_SERVICE } from "$lib/user/userService.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		Button,
		CardGroup,
		Modal,
		ProfilePictureUpload,
		Select,
		SelectItem,
		Spacer,
		Textbox,
		Toggle,
		chipToasts,
	} from "@gitbutler/ui-svelte";
	import { onMount } from "svelte";
	import type { User } from "$lib/user/user";

	const userService = inject(USER_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const projectsService = inject(PROJECTS_SERVICE);

	const updaterService = inject(UPDATER_SERVICE);
	const disableAutoChecks = updaterService.disableAutoChecks;

	const cliManager = inject(CLI_MANAGER);
	const [instalCLI, installingCLI] = cliManager.install;

	const backend = inject(BACKEND);
	const platformName = backend.platformName;

	const terminalService = inject(TERMINAL_SERVICE);

	const appSettings = settingsService.appSettings;

	let saving = $state(false);
	let newName = $state("");
	let isDeleting = $state(false);
	let loaded = $state(false);

	let userPicture = $state(userService.user?.picture);

	let deleteConfirmationModal: ReturnType<typeof Modal> | undefined = $state();

	const uiState = inject(UI_STATE);
	const defaultCodeEditor = uiState.global.defaultCodeEditor;
	const defaultTerminal = uiState.global.defaultTerminal;
	const localeSetting = uiState.global.locale;
	const localeOptions = APP_LOCALES.map((l) => ({ label: l.label, value: l.value }));

	const editorOptions: CodeEditorSettings[] = [
		{ schemeIdentifer: "vscodium", displayName: "VSCodium" },
		{ schemeIdentifer: "vscode", displayName: "VSCode" },
		{ schemeIdentifer: "vscode-insiders", displayName: "VSCode Insiders" },
		{ schemeIdentifer: "windsurf", displayName: "Windsurf" },
		{ schemeIdentifer: "zed", displayName: "Zed" },
		{ schemeIdentifer: "cursor", displayName: "Cursor" },
		{ schemeIdentifer: "trae", displayName: "Trae" },
		{ schemeIdentifer: "antigravity-ide", displayName: "Antigravity IDE" },
	];
	const editorOptionsForSelect = editorOptions.map((option) => ({
		label: option.displayName,
		value: option.schemeIdentifer,
	}));

	let terminalOptions: TerminalSettings[] = $state([]);
	let terminalOptionsForSelect: Array<{ label: string; value: string }> = $state([]);

	onMount(async () => {
		try {
			const options = await terminalService.getTerminalOptionsForPlatform(platformName);
			terminalOptions = options;
			terminalOptionsForSelect = options.map((option) => ({
				label: option.displayName,
				value: option.identifier,
			}));
		} catch (err) {
			console.error("Failed to load terminal options", err);
		}
	});

	$effect(() => {
		if (userService.user && !loaded) {
			loaded = true;
			userService.getUser().then((cloudUser) => {
				const userData: User = {
					...cloudUser,
					name: cloudUser.name || undefined,
					email: cloudUser.email || undefined,
					login: cloudUser.login || undefined,
					picture: cloudUser.picture || "#",
					locale: cloudUser.locale || "en",
					access_token: cloudUser.access_token || "impossible-situation",
					role: cloudUser.role || "user",
					supporter: cloudUser.supporter || false,
				};
				userPicture = userData.picture;
				userService.setUser(userData);
			});
			newName = userService.user?.name || "";
		}
	});

	let selectedPictureFile: File | undefined = $state();

	async function onSubmit(e: SubmitEvent) {
		if (!userService.user) return;
		saving = true;

		e.preventDefault();

		try {
			const updatedUser = await userService.updateUser({
				name: newName,
				picture: selectedPictureFile,
			});
			userService.setUser(updatedUser);
			chipToasts.success("Profile updated");
			selectedPictureFile = undefined;
		} finally {
			saving = false;
		}
	}

	function onPictureChange(file: File) {
		selectedPictureFile = file;
		userPicture = URL.createObjectURL(file);
	}

	async function onDeleteClicked() {
		isDeleting = true;
		try {
			await settingsService.deleteAllData();
			projectsService.unsetLastOpenedProject();
			await userService.forgetUserCredentials();
			chipToasts.success("All data deleted");
			goto("/", { replaceState: true, invalidateAll: true });
		} finally {
			deleteConfirmationModal?.close();
			isDeleting = false;
		}
	}

	let showSymlink = $state(false);
</script>

{#if userService.user}
	<CardGroup>
		<form onsubmit={onSubmit} class="profile-form">
			<ProfilePictureUpload
				bind:picture={userPicture}
				onFileSelect={onPictureChange}
				onInvalidFileType={() => chipToasts.error("Please use a valid image file")}
			/>

			<div id="contact-info" class="contact-info">
				<div class="contact-info__fields">
					<Textbox label={t("full-name")} bind:value={newName} required />
					<Textbox label="Email" value={userService.user?.email} readonly />
				</div>

				<Button type="submit" style="pop" loading={saving}>{t("update-profile")}</Button>
			</div>
		</form>
	</CardGroup>

	<CardGroup>
		<CardGroup.Item>
			{#snippet title()}
				{t("forget-credentials-and-log-out")}
			{/snippet}
			{#snippet caption()}
				{t("click-here-to-clear-your-credentials-and-unwind")}
			{/snippet}
			{#snippet actions()}
				<Button
					kind="outline"
					icon="logout"
					onclick={async () => {
						await userService.forgetUserCredentials();
					}}>{t("forget-credentials")}</Button
				>
			{/snippet}
		</CardGroup.Item>
	</CardGroup>
{/if}

<AccessTokenSignIn />

<Spacer />

<CardGroup>
	<CardGroup.Item alignment="center">
		{#snippet title()}
			{t("language")}
		{/snippet}
		{#snippet actions()}
			<Select
				value={localeSetting.current}
				options={localeOptions}
				onselect={(value) => {
					localeSetting.set(value as AppLocale);
				}}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === localeSetting.current} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		{/snippet}
	</CardGroup.Item>
	<CardGroup.Item alignment="center">
		{#snippet title()}
			{t("default-code-editor")}
		{/snippet}
		{#snippet actions()}
			<Select
				value={defaultCodeEditor.current.schemeIdentifer}
				options={editorOptionsForSelect}
				onselect={(value) => {
					const selected = editorOptions.find((option) => option.schemeIdentifer === value);
					if (selected) {
						defaultCodeEditor.set(selected);
					}
				}}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem
						selected={item.value === defaultCodeEditor.current.schemeIdentifer}
						{highlighted}
					>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		{/snippet}
	</CardGroup.Item>
	{#if platformName !== "web"}
		<CardGroup.Item alignment="center">
			{#snippet title()}
				{t("default-terminal")}
			{/snippet}
			{#snippet actions()}
				<Select
					value={defaultTerminal.current.identifier}
					options={terminalOptionsForSelect}
					onselect={(value) => {
						const selected = terminalOptions.find((option) => option.identifier === value);
						if (selected) {
							defaultTerminal.set(selected);
						}
					}}
				>
					{#snippet itemSnippet({ item, highlighted })}
						<SelectItem selected={item.value === defaultTerminal.current.identifier} {highlighted}>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>
			{/snippet}
		</CardGroup.Item>
	{/if}
</CardGroup>

<CardGroup>
	<CardGroup.Item labelFor="disable-auto-checks">
		{#snippet title()}
			{t("automatically-check-for-updates")}
		{/snippet}

		{#snippet caption()}
			{t("automatically-check-for-updates-you-can-still-check-manually")}
		{/snippet}

		{#snippet actions()}
			<Toggle
				id="disable-auto-checks"
				checked={!$disableAutoChecks}
				onclick={() => ($disableAutoChecks = !$disableAutoChecks)}
			/>
		{/snippet}
	</CardGroup.Item>
</CardGroup>

<CardGroup>
	<CardGroup.Item>
		{#snippet title()}
			{t("install-the-gitbutler-cli")} <code class="code-string">but</code>
		{/snippet}

		{#snippet caption()}
			{#if $appSettings?.ui.cliIsManagedByPackageManager}
				The <code>but</code> {t("cli-is-managed-by-your-package-manager-please-use-your-packa")}
			{:else if platformName === "windows"}
				{t("on-windows-you-can-manually-copy-the-executable")}<code>`but`</code>{t(
					"to-a-directory-in-your-path-click-show-command-for-instructi",
				)}
			{:else if platformName === "linux"}
				{t("on-linux-you-can-manually-create-a-symlink-to-the-cli-in-you")}
			{:else}
				{t("installs-the-gitbutler-cli")}<code>`but`</code>{t(
					"in-your-path-allowing-you-to-use-it-from-the-terminal-this-a",
				)}
			{/if}
		{/snippet}

		{#if !$appSettings?.ui.cliIsManagedByPackageManager}
			<div class="flex flex-col gap-16">
				<div class="flex gap-8 justify-end">
					{#if platformName === "macos"}
						<Button
							style="pop"
							icon="play"
							onclick={async () => {
								try {
									await instalCLI();
								} catch (err: unknown) {
									// osascript returns a generic non-success when the
									// user dismisses the macOS admin-privileges prompt.
									// The backend tags that specific case with a
									// `CliInstallCancelled` code so we can show an info
									// toast instead of an error toast.
									if (getUserErrorCode(err) === "CliInstallCancelled") {
										showToast({
											style: "info",
											message: "CLI install cancelled.",
										});
										return;
									}
									throw err;
								}
							}}
							loading={installingCLI.current.isLoading}
						>
							{t("install-but-cli")}</Button
						>
					{/if}
					<Button
						style="gray"
						kind="outline"
						disabled={showSymlink}
						onclick={() => (showSymlink = !showSymlink)}>{t("show-command")}</Button
					>
				</div>
			</div>

			{#if showSymlink}
				<CliSymlinkSetup class="m-t-14" />
			{/if}
		{/if}
	</CardGroup.Item>
</CardGroup>

<Spacer />

<CardGroup>
	<CardGroup.Item>
		{#snippet title()}
			{t("remove-all-projects")}
		{/snippet}
		{#snippet caption()}
			{t("you-can-delete-all-projects-from-the-gitbutler-app")}
			<br />
			{t("your-code-remains-safe-it-only-clears-the-configuration")}
		{/snippet}

		{#snippet actions()}
			<Button style="danger" kind="outline" onclick={() => deleteConfirmationModal?.show()}>
				{t("remove-projects")}
			</Button>
		{/snippet}
	</CardGroup.Item>
</CardGroup>

<Modal
	bind:this={deleteConfirmationModal}
	width="small"
	title="Remove all projects"
	onSubmit={onDeleteClicked}
>
	<p>{t("are-you-sure-you-want-to-remove-all-gitbutler-projects")}</p>

	{#snippet controls(close)}
		<Button style="danger" kind="outline" loading={isDeleting} type="submit">Remove</Button>
		<Button style="pop" onclick={close}>Cancel</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.profile-form {
		display: flex;
		padding: 16px;
		gap: 24px;
	}

	.contact-info {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: flex-end;
		gap: 20px;
	}

	.contact-info__fields {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 12px;
	}
</style>

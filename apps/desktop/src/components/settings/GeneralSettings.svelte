<script lang="ts">
	import { goto } from "$app/navigation";
	import CliSymlinkSetup from "$components/settings/CliSymlinkSetup.svelte";
	import AccessTokenSignIn from "$components/shared/AccessTokenSignIn.svelte";
	import { BACKEND } from "$lib/backend";
	import { getUserErrorCode } from "$lib/backend/ipc";
	import { CLI_MANAGER } from "$lib/config/cli";
	import { showError } from "$lib/error/showError";
	import { showToast } from "$lib/notifications/toasts";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
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
	} from "@gitbutler/ui";
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

	const CUSTOM_EDITOR_VALUE = "__custom__";
	const CUSTOM_EDITOR_SCHEME_PATTERN = /^[A-Za-z][A-Za-z0-9+.-]*$/;

	const editorOptions: CodeEditorSettings[] = [
		{ schemeIdentifer: "vscodium", displayName: "VSCodium" },
		{ schemeIdentifer: "vscode", displayName: "VSCode" },
		{ schemeIdentifer: "vscode-insiders", displayName: "VSCode Insiders" },
		{ schemeIdentifer: "windsurf", displayName: "Windsurf" },
		{ schemeIdentifer: "zed", displayName: "Zed" },
		{ schemeIdentifer: "cursor", displayName: "Cursor" },
		{ schemeIdentifer: "trae", displayName: "Trae" },
		{ schemeIdentifer: "jetbrains-idea", displayName: "IntelliJ IDEA" },
		{ schemeIdentifer: "jetbrains-webstorm", displayName: "WebStorm" },
		{ schemeIdentifer: "jetbrains-pycharm", displayName: "PyCharm" },
		{ schemeIdentifer: "jetbrains-clion", displayName: "CLion" },
		{ schemeIdentifer: "jetbrains-goland", displayName: "GoLand" },
		{ schemeIdentifer: "jetbrains-phpstorm", displayName: "PhpStorm" },
		{ schemeIdentifer: "jetbrains-rider", displayName: "Rider" },
		{ schemeIdentifer: "jetbrains-rubymine", displayName: "RubyMine" },
		{ schemeIdentifer: "jetbrains-datagrip", displayName: "DataGrip" },
		{ schemeIdentifer: "jetbrains-dataspell", displayName: "DataSpell" },
	];
	const editorOptionsForSelect = [
		...editorOptions.map((option) => ({
			label: option.displayName,
			value: option.schemeIdentifer,
		})),
		{ label: "Custom", value: CUSTOM_EDITOR_VALUE },
	];

	let customEditorName = $state(
		editorOptions.some(
			(option) => option.schemeIdentifer === defaultCodeEditor.current.schemeIdentifer,
		)
			? "Custom editor"
			: defaultCodeEditor.current.displayName,
	);
	let customEditorScheme = $state(
		editorOptions.some(
			(option) => option.schemeIdentifer === defaultCodeEditor.current.schemeIdentifer,
		)
			? ""
			: defaultCodeEditor.current.schemeIdentifer,
	);
	let selectedEditorValue = $state(
		editorOptions.some(
			(option) => option.schemeIdentifer === defaultCodeEditor.current.schemeIdentifer,
		)
			? defaultCodeEditor.current.schemeIdentifer
			: CUSTOM_EDITOR_VALUE,
	);
	const customEditorSchemeError = $derived(
		customEditorScheme && !CUSTOM_EDITOR_SCHEME_PATTERN.test(customEditorScheme)
			? "Use a URI scheme such as code, emacs, or my-editor."
			: undefined,
	);

	const allTerminalOptions: TerminalSettings[] = [
		// macOS
		{ identifier: "terminal", displayName: "Terminal", platform: "macos" },
		{ identifier: "iterm2", displayName: "iTerm2", platform: "macos" },
		{ identifier: "ghostty", displayName: "Ghostty", platform: "macos" },
		{ identifier: "warp", displayName: "Warp", platform: "macos" },
		{ identifier: "alacritty-mac", displayName: "Alacritty", platform: "macos" },
		{ identifier: "wezterm-mac", displayName: "WezTerm", platform: "macos" },
		{ identifier: "hyper", displayName: "Hyper", platform: "macos" },
		{ identifier: "kitty", displayName: "Kitty", platform: "macos" },
		// Windows
		{ identifier: "wt", displayName: "Windows Terminal", platform: "windows" },
		{ identifier: "powershell", displayName: "PowerShell", platform: "windows" },
		{ identifier: "cmd", displayName: "Command Prompt", platform: "windows" },
		// Linux
		{ identifier: "gnome-terminal", displayName: "GNOME Terminal", platform: "linux" },
		{ identifier: "konsole", displayName: "Konsole", platform: "linux" },
		{ identifier: "xfce4-terminal", displayName: "XFCE Terminal", platform: "linux" },
		{ identifier: "alacritty", displayName: "Alacritty", platform: "linux" },
		{ identifier: "ghostty", displayName: "Ghostty", platform: "linux" },
		{ identifier: "warp", displayName: "Warp", platform: "linux" },
		{ identifier: "hyper", displayName: "Hyper", platform: "linux" },
		{ identifier: "wezterm", displayName: "WezTerm", platform: "linux" },
		{ identifier: "kitty", displayName: "Kitty", platform: "linux" },
		{ identifier: "cosmic-term", displayName: "COSMIC Terminal", platform: "linux" },
		{ identifier: "ptyxis", displayName: "Ptyxis", platform: "linux" },
	];

	const terminalOptions = allTerminalOptions.filter((t) => t.platform === platformName);
	const terminalOptionsForSelect = terminalOptions.map((option) => ({
		label: option.displayName,
		value: option.identifier,
	}));

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
		} catch (err: any) {
			console.error(err);
			showError("Failed to update user", err);
		}
		saving = false;
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
		} catch (err: any) {
			console.error(err);
			showError("Failed to delete project", err);
		} finally {
			deleteConfirmationModal?.close();
			isDeleting = false;
		}
	}

	let showSymlink = $state(false);

	function saveCustomEditor() {
		const scheme = customEditorScheme.trim();
		if (!scheme || !CUSTOM_EDITOR_SCHEME_PATTERN.test(scheme)) return;
		defaultCodeEditor.set({
			displayName: customEditorName.trim() || "Custom editor",
			schemeIdentifer: scheme,
		});
	}
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
					<Textbox label="Full name" bind:value={newName} required />
					<Textbox label="Email" value={userService.user?.email} readonly />
				</div>

				<Button type="submit" style="pop" loading={saving}>Update profile</Button>
			</div>
		</form>
	</CardGroup>

	<CardGroup>
		<CardGroup.Item>
			{#snippet title()}
				Forget credentials and log out
			{/snippet}
			{#snippet caption()}
				Click here to clear your credentials and unwind.
			{/snippet}
			{#snippet actions()}
				<Button
					kind="outline"
					icon="logout"
					onclick={async () => {
						await userService.forgetUserCredentials();
					}}>Forget credentials</Button
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
			Default code editor
		{/snippet}
		{#snippet actions()}
			<div class="editor-settings">
				<Select
					value={selectedEditorValue}
					options={editorOptionsForSelect}
					onselect={(value) => {
						if (value === CUSTOM_EDITOR_VALUE) {
							selectedEditorValue = CUSTOM_EDITOR_VALUE;
							if (!customEditorScheme) {
								customEditorScheme = defaultCodeEditor.current.schemeIdentifer;
							}
							return;
						}

						const selected = editorOptions.find((option) => option.schemeIdentifer === value);
						if (selected) {
							selectedEditorValue = value;
							defaultCodeEditor.set(selected);
						}
					}}
				>
					{#snippet itemSnippet({ item, highlighted })}
						<SelectItem selected={item.value === selectedEditorValue} {highlighted}>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>
				{#if selectedEditorValue === CUSTOM_EDITOR_VALUE}
					<div class="custom-editor-fields">
						<Textbox
							label="Name"
							bind:value={customEditorName}
							placeholder="Custom editor"
							onchange={saveCustomEditor}
						/>
						<Textbox
							label="URI scheme"
							bind:value={customEditorScheme}
							placeholder="my-editor"
							error={customEditorSchemeError}
							helperText="GitButler opens files as scheme://file/path:line:column."
							onchange={saveCustomEditor}
						/>
						<Button
							style="pop"
							disabled={!customEditorScheme || !!customEditorSchemeError}
							onclick={saveCustomEditor}>Save</Button
						>
					</div>
				{/if}
			</div>
		{/snippet}
	</CardGroup.Item>
	{#if platformName !== "web"}
		<CardGroup.Item alignment="center">
			{#snippet title()}
				Default terminal
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
			Automatically check for updates
		{/snippet}

		{#snippet caption()}
			Automatically check for updates. You can still check manually when needed.
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
			Install the GitButler CLI <code class="code-string">but</code>
		{/snippet}

		{#snippet caption()}
			{#if $appSettings?.ui.cliIsManagedByPackageManager}
				The <code>but</code> CLI is managed by your package manager. Please use your package manager to
				install, update, or remove it.
			{:else if platformName === "windows"}
				On Windows, you can manually copy the executable (<code>`but`</code>) to a directory in your
				PATH. Click "Show Command" for instructions.
			{:else}
				Installs the GitButler CLI (<code>`but`</code>) in your PATH, allowing you to use it from
				the terminal. This action will request admin privileges. Alternatively, you could create a
				symlink manually.
			{/if}
		{/snippet}

		{#if !$appSettings?.ui.cliIsManagedByPackageManager}
			<div class="flex flex-col gap-16">
				<div class="flex gap-8 justify-end">
					{#if platformName !== "windows"}
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
							Install But CLI</Button
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

<Spacer />

<CardGroup>
	<CardGroup.Item>
		{#snippet title()}
			Remove all projects
		{/snippet}
		{#snippet caption()}
			You can delete all projects from the GitButler app.
			<br />
			Your code remains safe. it only clears the configuration.
		{/snippet}

		{#snippet actions()}
			<Button style="danger" kind="outline" onclick={() => deleteConfirmationModal?.show()}>
				Remove projects…
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
	<p>Are you sure you want to remove all GitButler projects?</p>

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

	.editor-settings {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 12px;
	}

	.custom-editor-fields {
		display: grid;
		grid-template-columns: minmax(140px, 1fr) minmax(180px, 1fr) auto;
		align-items: end;
		gap: 8px;
	}
</style>

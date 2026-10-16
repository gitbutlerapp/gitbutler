import { expect, type ElectronApplication, type Locator, type Page } from "@playwright/test";
import { Backend } from "./backend.ts";

export type NativeMenuItem = {
	_tag: "Item" | "Separator";
	enabled?: boolean;
	itemId?: string;
	label?: string;
};

/** Waits on git operations, which outlast the default on a busy machine. */
const gitTimeout = 15_000;

/** The Lite window, described by what a user sees in it. */
export class App {
	readonly page: Page;
	readonly electronApp: ElectronApplication;
	readonly backend: Backend;

	constructor(page: Page, electronApp: ElectronApplication) {
		this.page = page;
		this.electronApp = electronApp;
		this.backend = new Backend(electronApp);
	}

	getByRole: Page["getByRole"] = (...args) => this.page.getByRole(...args);
	getByText: Page["getByText"] = (...args) => this.page.getByText(...args);
	getByLabel: Page["getByLabel"] = (...args) => this.page.getByLabel(...args);

	async press(key: string): Promise<void> {
		await this.page.keyboard.press(key);
	}

	/** The platform's delete shortcut, which deletes the selection. */
	async pressDelete(): Promise<void> {
		await this.press(process.platform === "darwin" ? "Meta+Backspace" : "Delete");
	}

	/** Stop the app noticing repository changes, so only its own updates show. */
	async stopWatchingFiles(): Promise<void> {
		await this.page.evaluate(() => window.lite.watcherStopAll());
	}

	/** Wait for the app to settle on a project's workspace or on onboarding. */
	async waitUntilReady(): Promise<void> {
		await this.page.getByRole("main").waitFor();
		await this.projectPicker.or(this.onboardingHeading).waitFor();
	}

	get projectId(): string {
		const projectId = new URL(this.page.url()).pathname.split("/")[2];
		if (projectId === undefined) throw new Error(`No project in ${this.page.url()}`);
		return projectId;
	}

	async reload(): Promise<void> {
		await this.page.reload();
		await this.waitUntilReady();
	}

	pageTab(name: "Workspace" | "Branches"): Locator {
		return this.page
			.getByRole("group", { name: "Pages" })
			.getByRole("button", { name, exact: true });
	}

	async goTo(page: "Workspace" | "Branches"): Promise<void> {
		await this.pageTab(page).click();
	}

	/** The list the cursor is in. */
	activeList(): string {
		return new URL(this.page.url()).searchParams.get("active") ?? "applied";
	}

	focusScope(name: string): Locator {
		return this.page.locator(`[data-focus-scope="${name}"]`);
	}

	/** The focus scope holding the focused element, if any. */
	focusedScope(): Promise<string | null> {
		return this.page.evaluate(
			() =>
				document.activeElement?.closest("[data-focus-scope]")?.getAttribute("data-focus-scope") ??
				null,
		);
	}

	async applyBranch(name: string): Promise<void> {
		const picker = this.page.getByRole("dialog", { name: "Apply branch" });
		// The shortcut is ignored until the window has settled.
		await expect(async () => {
			await this.page.keyboard.press("ControlOrMeta+Shift+A");
			await expect(picker).toBeVisible({ timeout: 500 });
		}).toPass({ timeout: 5_000 });
		await picker.getByRole("combobox", { name: /search for branches/i }).fill(name);
		await picker.getByRole("option", { name: new RegExp(`^${name} `) }).click();
		await expect(picker).toBeHidden();
	}

	/** Make the native open dialog choose `path` from now on. */
	async answerOpenDialogsWith(path: string): Promise<void> {
		await this.electronApp.evaluate(({ dialog }, path) => {
			dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [path] });
		}, path);
	}

	get onboardingHeading(): Locator {
		return this.page.getByRole("heading", { name: /^Welcome to GitButler/ });
	}

	get projectPicker(): Locator {
		return this.page.getByRole("combobox", { name: /Select project/ });
	}

	/** The short name of the branch checked out outside the workspace, if any. */
	checkedOutBranch(): Promise<string | null> {
		return this.page.evaluate(async (projectId) => {
			const { operatingMode } = await window.lite.operatingMode(projectId);
			return operatingMode.type === "OutsideWorkspace" ? operatingMode.subject.branchName : null;
		}, this.projectId);
	}

	/**
	 * Click `trigger` and pick `label` from the native menu it opens, which
	 * Playwright cannot see. Returns the menu's items.
	 */
	async chooseFromNativeMenu(trigger: Locator, label: string): Promise<Array<NativeMenuItem>> {
		await this.electronApp.evaluate(({ ipcMain }, label) => {
			const state = globalThis as { nativeMenuItems?: Array<NativeMenuItem> };
			delete state.nativeMenuItems;
			ipcMain.removeHandler("showNativeMenu");
			ipcMain.handle("showNativeMenu", (_event, params: { items: Array<NativeMenuItem> }) => {
				state.nativeMenuItems = params.items;
				const item = params.items.find((item) => item.label === label);
				return item?.enabled === false ? null : (item?.itemId ?? null);
			});
		}, label);
		await trigger.click();
		const items = () =>
			this.electronApp.evaluate(
				() => (globalThis as { nativeMenuItems?: Array<NativeMenuItem> }).nativeMenuItems ?? [],
			);
		await expect.poll(items).not.toEqual([]);
		return items();
	}

	branch(name: string): Locator {
		return this.page.getByRole("treeitem", { name, exact: true });
	}

	/** The branch row's own line, without the commits nested in its tree item. */
	branchName(name: string): Locator {
		return this.branch(name).getByTitle(name, { exact: true });
	}

	branchMenu(name: string): Locator {
		return this.branch(name).getByRole("button", { name: "Branch menu", exact: true });
	}

	async selectBranch(name: string): Promise<void> {
		await this.branchName(name).click();
		await expect(this.branch(name)).toHaveAttribute("aria-selected", "true");
	}

	/** Select the branch and open its name editor. */
	async editBranchName(name: string): Promise<Locator> {
		await this.selectBranch(name);
		await this.page.keyboard.press("F2");
		return this.branch(name).getByRole("textbox", { name: "Branch name" });
	}

	async renameBranch(from: string, to: string): Promise<void> {
		const editor = await this.editBranchName(from);
		await editor.fill(to);
		await editor.press("Enter");
		await expect(this.branch(to)).toBeVisible({ timeout: gitTimeout });
	}

	commit(title: string): Locator {
		return this.page.getByRole("treeitem", { name: title, exact: true });
	}

	async selectCommit(title: string): Promise<void> {
		await this.commit(title).click();
		await expect(this.commit(title)).toHaveAttribute("aria-selected", "true");
	}

	/** Open the commit's inline message editor. */
	async editCommitMessage(title: string): Promise<Locator> {
		const row = this.commit(title);
		await row.getByText(title, { exact: true }).dblclick();
		const editor = row.getByRole("textbox", { name: "Commit message" });
		await expect(editor).toBeFocused();
		return editor;
	}

	async rewordCommit(from: string, to: string): Promise<void> {
		const editor = await this.editCommitMessage(from);
		await editor.fill(to);
		await editor.press("Enter");
	}

	/** Show the selected commit's file list next to its diff. */
	async showCommitFiles(): Promise<void> {
		const toggle = this.page.getByRole("button", { name: "Toggle files" });
		if ((await toggle.getAttribute("aria-pressed")) !== "true") await toggle.click();
	}

	get uncommitted(): Locator {
		return this.page.getByRole("tree", { name: "Uncommitted" });
	}

	uncommittedFile(path: string): Locator {
		return this.uncommitted.getByRole("treeitem", { name: `Addition ${path}` });
	}

	fileCheckbox(path: string): Locator {
		return this.uncommitted.getByRole("checkbox", { name: `Check file ${path}` });
	}

	get startCommitButton(): Locator {
		return this.page.getByRole("button", { name: /start commit/i });
	}

	/** Open the commit form; returns its message field. */
	async startCommit(): Promise<Locator> {
		await this.startCommitButton.click();
		const message = this.page.getByRole("textbox", { name: "Compose commit message" });
		await expect(message).toBeFocused();
		return message;
	}

	async commitFiles(message: string, ...paths: Array<string>): Promise<void> {
		for (const path of paths) await this.fileCheckbox(path).click();
		await (await this.startCommit()).fill(message);
		await this.page.getByRole("button", { name: "Commit", exact: true }).click();
		await expect(this.commit(message)).toBeVisible();
	}

	get editModeHeading(): Locator {
		return this.page.getByRole("heading", { name: "Editing commit" });
	}

	/** Edit the workspace's first commit, the way its commit menu does. */
	async enterEditMode(): Promise<void> {
		await this.page.evaluate(async (projectId) => {
			const { stacks } = await window.lite.headInfo(projectId);
			const stackId = stacks[0]?.id;
			const commit = stacks[0]?.segments.flatMap((segment) => segment.commits)[0];
			if (stackId === undefined || stackId === null || commit === undefined)
				throw new Error("The seeded project has no commit to edit");
			await window.lite.enterEditMode({ projectId, commitId: commit.id, stackId });
		}, this.projectId);
		await expect(this.editModeHeading).toBeVisible();
	}

	/**
	 * Where the scrolling list holding `row` stands, after scrolling it to `top`
	 * if given. The list virtualises, so `row` only anchors the lookup.
	 */
	listScroll(row: Locator, top?: number | "end"): Promise<{ offset: number; end: number }> {
		return row.evaluate((row, top) => {
			let scroller = row.parentElement;
			while (scroller && !["auto", "scroll"].includes(getComputedStyle(scroller).overflowY))
				scroller = scroller.parentElement;
			if (!scroller) throw new Error("The row is in no scrolling list");
			if (top !== undefined) scroller.scrollTop = top === "end" ? scroller.scrollHeight : top;
			return {
				offset: Math.round(scroller.scrollTop),
				end: Math.round(scroller.scrollHeight - scroller.clientHeight),
			};
		}, top);
	}

	/**
	 * Wheel over `row` enough to cross its list several times over. Few, large
	 * steps: each is a round trip, and a stuck list stays stuck however finely
	 * it is asked to move.
	 */
	async wheelThroughList(row: Locator): Promise<void> {
		await row.hover();
		for (let step = 0; step < 16; step++) await this.page.mouse.wheel(0, 400);
	}

	async isScrolledToEnd(row: Locator): Promise<boolean> {
		const { offset, end } = await this.listScroll(row);
		return offset >= end;
	}

	branchTab(name: string): Locator {
		return this.page
			.getByRole("tablist", { name: "Branch tab", exact: true })
			.getByRole("tab", { name, exact: true });
	}

	get pullRequestForm(): Locator {
		return this.page.getByPlaceholder("PR title");
	}

	/** The merge checklist on a branch's pull request tab. */
	get checklist(): Locator {
		return this.page.getByRole("region", { name: "Checklist", exact: true });
	}

	get mergeButton(): Locator {
		return this.page.getByRole("button", { name: "Merge", exact: true });
	}

	async addTodo(label: string): Promise<void> {
		await this.checklist.getByRole("button", { name: "Add to-do", exact: true }).click();
		await this.checklist.getByRole("textbox", { name: "To-do" }).fill(label);
		await this.page.keyboard.press("ControlOrMeta+Enter");
		// A new to-do stays disabled until it is saved.
		await expect(this.todo(label)).toBeEnabled();
	}

	get todos(): Locator {
		return this.checklist.getByRole("checkbox");
	}

	async removeTodo(label: string): Promise<void> {
		await this.checklist.getByRole("button", { name: `Remove ${label}`, exact: true }).click();
	}

	reviewComment(id: number): Locator {
		return this.page.locator(`#review-comment-${id}`);
	}

	/** Tick a to-do; its box only shows as ticked once the change is saved. */
	async tickTodo(label: string): Promise<void> {
		await this.todo(label).click();
		await expect(this.todo(label)).toBeChecked();
	}

	todo(label: string): Locator {
		return this.checklist.getByRole("checkbox", { name: label });
	}

	get incomingCommits(): Locator {
		return this.page.getByRole("button", { name: /incoming commit/ });
	}

	updateButton(branch: string): Locator {
		return this.page.getByLabel(`Integrate origin/${branch} into ${branch}`);
	}

	async openBranchUpdate(branch: string): Promise<BranchUpdate> {
		await this.updateButton(branch).click();
		// The summary renders once the dry-run preview for the default answer is in.
		await expect(this.page.getByText(/incoming commit/)).toBeVisible();
		return new BranchUpdate(this.page);
	}
}

type UpdateChoice = "Keep mine" | "Combine both" | "Take theirs";

const updateAction = {
	"Keep mine": { button: "Force push", toast: "Branch published." },
	"Combine both": { button: "Integrate", toast: "Branch updated." },
	"Take theirs": { button: "Replace branch", toast: "Branch replaced." },
} as const;

/** The update-from-remote dialog. */
export class BranchUpdate {
	readonly dialog: Locator;
	private readonly page: Page;

	constructor(page: Page) {
		this.page = page;
		this.dialog = page.locator('[aria-labelledby="branch-update-heading"]');
	}

	button(name: string): Locator {
		return this.dialog.getByRole("button", { name, exact: true });
	}

	/** Untick one of their commits so the integration leaves it out. */
	async leaveOut(subject: string): Promise<void> {
		await this.page.getByLabel(`Keep ${subject}`).click();
		// The preview recounts before the action takes the new selection.
		await expect(this.page.getByText(/\d+ of \d+ incoming commits/)).toBeVisible();
	}

	async apply(choice: UpdateChoice): Promise<void> {
		const chip = this.button(choice);
		// A pressed toggle does not take another click.
		if ((await chip.getAttribute("aria-pressed")) !== "true") await chip.click();
		const { button, toast } = updateAction[choice];
		// Disabled while the outline still shows the previous choice's data.
		await expect(this.button(button)).toBeEnabled();
		await this.button(button).click();
		await expect(this.page.getByText(toast)).toBeVisible({ timeout: gitTimeout });
	}

	/** Leave the done state, publishing first unless `action` is Done. */
	async finish(action: "Done" | "Push" | "Force push" = "Done"): Promise<void> {
		if (action !== "Done") {
			await this.button(action).click();
			await expect(this.page.getByText("Branch published.")).toBeVisible({ timeout: gitTimeout });
		}
		await this.button("Done").click();
		await expect(this.dialog).toBeHidden();
	}
}

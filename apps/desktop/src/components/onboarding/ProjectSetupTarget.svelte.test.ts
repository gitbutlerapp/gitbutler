import ProjectSetupTarget from "$components/onboarding/ProjectSetupTarget.svelte";
import { GIT_CONFIG_SERVICE } from "$lib/config/gitConfigService";
import { PROJECTS_SERVICE } from "$lib/project/projectsService";
import { POSTHOG_WRAPPER } from "$lib/telemetry/posthog";
import { render, screen, waitFor } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

function renderTarget(
	onBranchSelected: (branch: [branchName: string, pushRemote: string | undefined]) => Promise<void>,
	onOpenProject = vi.fn(async () => false),
	remoteBranches = [{ name: "origin/main" }],
	notice?: string,
) {
	const context = new Map<any, any>([
		[
			GIT_CONFIG_SERVICE._key,
			{
				gbConfig: () => ({ response: { gitbutlerGerritMode: false } }),
				setGerritMode: vi.fn(),
			},
		],
		[
			PROJECTS_SERVICE._key,
			{
				areYouGerritKiddingMe: () => ({ result: { data: false, status: "fulfilled" } }),
				isGerritProject: () => ({ result: { data: false, status: "fulfilled" } }),
				deleteProject: vi.fn(),
				fetchProjects: vi.fn(),
				projects: () => ({ response: [{ id: "project-id", title: "Repository" }] }),
				serverCapabilities: () => ({ response: { canAddProjects: true } }),
			},
		],
		[POSTHOG_WRAPPER._key, { captureOnboarding: vi.fn() }],
	]);

	return render(ProjectSetupTarget, {
		props: {
			projectId: "project-id",
			projectName: "Repository",
			remoteBranches,
			localBranches: [{ name: "release", refName: "refs/heads/release", isTarget: false }],
			onBranchSelected,
			onOpenProject,
			notice,
		},
		context,
	});
}

test("says why the target needs choosing again", () => {
	renderTarget(
		vi.fn(),
		vi.fn(async () => false),
		[{ name: "origin/main" }],
		"No local branch tracks your current target, upstream/main.",
	);
	expect(
		screen.getByText("No local branch tracks your current target, upstream/main."),
	).toBeInTheDocument();
});

test("offers other projects rather than removing this one when replacing a target", () => {
	renderTarget(
		vi.fn(),
		vi.fn(async () => false),
		[{ name: "origin/main" }],
		"No local branch tracks your current target, upstream/main.",
	);
	expect(screen.queryByRole("button", { name: "Cancel" })).not.toBeInTheDocument();
	expect(screen.getByText("Switch to another project")).toBeInTheDocument();
});

test("keeps the push remote when replacing a target", async () => {
	const onBranchSelected = vi.fn();
	renderTarget(
		onBranchSelected,
		vi.fn(async () => false),
		[{ name: "origin/main" }, { name: "fork/main" }],
		"No local branch tracks your current target, upstream/main.",
	);
	expect(screen.queryByText(/You have branches from multiple remotes/)).not.toBeInTheDocument();
	await userEvent.setup().click(screen.getByRole("button", { name: "Let's go" }));
	expect(onBranchSelected).toHaveBeenCalledWith(["refs/heads/release", undefined]);
});

test("submits the local ref independently of the publication remote", async () => {
	const onBranchSelected = vi.fn();
	renderTarget(onBranchSelected);
	await userEvent.setup().click(screen.getByRole("button", { name: "Let's go" }));
	expect(onBranchSelected).toHaveBeenCalledWith(["refs/heads/release", "origin"]);
});

test("sets a local target without a remote", async () => {
	const onBranchSelected = vi.fn();
	renderTarget(
		onBranchSelected,
		vi.fn(async () => false),
		[],
	);
	await userEvent.setup().click(screen.getByRole("button", { name: "Let's go" }));
	expect(onBranchSelected).toHaveBeenCalledWith(["refs/heads/release", undefined]);
});

test("does not promise an automatic workspace checkout", () => {
	renderTarget(vi.fn());
	expect(screen.queryByText(/GitButler switches your active branch/)).not.toBeInTheDocument();
});

test("allows only one target submission while the request is pending", async () => {
	let resolvePending!: () => void;
	const pending = new Promise<void>((resolve) => {
		resolvePending = resolve;
	});
	const onBranchSelected = vi.fn(async () => await pending);
	const onOpenProject = vi.fn(async () => false);
	const user = userEvent.setup();
	renderTarget(onBranchSelected, onOpenProject);

	const submit = screen.getByRole("button", { name: "Let's go" });
	const cancel = screen.getByRole("button", { name: "Cancel" });
	await user.click(submit);
	await user.click(submit);

	expect(onBranchSelected).toHaveBeenCalledTimes(1);
	expect(submit).toBeDisabled();
	expect(cancel).toBeDisabled();

	resolvePending();
	await pending;
	await waitFor(() => {
		expect(screen.getByRole("button", { name: "Open project" })).toBeEnabled();
		expect(cancel).toBeEnabled();
	});
	expect(onOpenProject).toHaveBeenCalledTimes(1);

	await user.click(screen.getByRole("button", { name: "Open project" }));
	expect(onBranchSelected).toHaveBeenCalledTimes(1);
	expect(onOpenProject).toHaveBeenCalledTimes(2);
});

test("shows an actionable target error and permits a retry", async () => {
	const message =
		"The selected target has no common history with HEAD. Fetch more history or choose another branch.";
	const onBranchSelected = vi
		.fn<(_: [branchName: string, pushRemote: string | undefined]) => Promise<void>>()
		.mockRejectedValueOnce(
			Object.assign(new Error(message), {
				code: "PreconditionFailed",
			}),
		)
		.mockResolvedValueOnce();
	const user = userEvent.setup();
	const { container } = renderTarget(onBranchSelected);

	const submit = screen.getByRole("button", { name: "Let's go" });
	await user.click(submit);

	expect(await screen.findByText(message)).toBeVisible();
	expect(screen.getAllByText(message)).toHaveLength(1);
	expect(container.querySelector(".info-message.warning")).toBeTruthy();
	expect(container.querySelector(".info-message.danger")).toBeNull();
	expect(submit).toBeEnabled();

	await user.click(submit);
	expect(onBranchSelected).toHaveBeenCalledTimes(2);
});

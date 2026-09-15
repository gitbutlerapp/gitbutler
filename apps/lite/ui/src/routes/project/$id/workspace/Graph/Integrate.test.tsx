/** @vitest-environment jsdom */
import {
	headInfoQueryOptions,
	workspaceTargetCommitsQueryOptions,
	changesInWorktreeQueryOptions,
} from "#ui/api/queries.ts";
import type { RefInfo, WorkspaceIntegrateUpstreamOutcome } from "@gitbutler/but-sdk";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Integrate, IntegrationStatus } from "./Integrate.tsx";
import { useWorkspaceIntegrationPreview } from "./useWorkspaceIntegrationPreview.ts";
import { assert } from "#ui/assert.ts";

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
vi.mock("#ui/components/Kbd.tsx", () => ({ Kbd: () => null }));
vi.mock("#ui/components/Tooltip.tsx", () => ({
	TooltipPopup: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}));
const state = vi.hoisted(() => ({ pending: false, integrate: vi.fn() }));
vi.mock("#ui/api/mutations.ts", () => ({
	useWorkspaceIntegrateUpstream: () => ({ isPending: false, mutate: state.integrate }),
}));
vi.mock("#ui/projects/state.ts", () => ({
	projectSlice: {
		selectors: { selectPendingOperation: () => ({ _tag: state.pending ? "Some" : "None" }) },
	},
}));
vi.mock("#ui/store.ts", () => ({
	useAppSelector: (select: (state: object) => unknown) => select({}),
}));

const projectId = "test-project";
const head = {
	stacks: [
		{
			id: "stack",
			base: "base",
			segments: [
				{
					refName: { displayName: "feature/login" },
					commits: [{ id: "local", changeId: "change", message: "Refresh sessions" }],
					base: "base",
				},
			],
		},
	],
	target: { isCurrent: false },
	worktrees: [],
} as unknown as RefInfo;
const outcome = (conflict: boolean, worktreeConflicts: Array<string> = []) =>
	({
		workspaceState: {
			headInfo: {
				stacks: [
					{
						segments: [
							{ commits: [{ id: "preview", message: "Refresh sessions", hasConflicts: conflict }] },
						],
					},
				],
			},
			checkoutConflictOccurred: false,
			replacedCommits: { local: "preview" },
		},
		commitConflicts: conflict ? { preview: ["src/session.ts", "assets/logo.png"] } : {},
		worktreeConflicts,
		targetCommits: null,
	}) as unknown as WorkspaceIntegrateUpstreamOutcome;
let client: QueryClient;
let root: Root;
let container: HTMLDivElement;
const preview = vi.fn();
const warning = () => container.querySelector('[aria-label="Update will cause conflicts"]');
const action = () =>
	assert(
		Array.from(container.querySelectorAll("button")).find(
			(button) => button.textContent === "Pull latest",
		),
	);
const Control = ({ projectId }: { projectId: string }) => {
	const preview = useWorkspaceIntegrationPreview(projectId);
	return (
		<>
			<IntegrationStatus target="origin/main" preview={preview} />
			<Integrate target="origin/main" preview={preview} />
		</>
	);
};
const render = async (id = projectId) => {
	await act(async () => {
		root.render(
			<QueryClientProvider client={client}>
				<Control projectId={id} />
			</QueryClientProvider>,
		);
	});
};
const flush = async (ms = 350) => {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(ms);
	});
};
const reviseHead = async (id: string) => {
	const current = structuredClone(head);
	assert(assert(assert(current.stacks[0]).segments[0]).commits[0]).id = id;
	await act(async () => {
		client.setQueryData(headInfoQueryOptions(projectId).queryKey, current, {
			updatedAt: Date.now() + 1,
		});
	});
	await flush(0);
};
beforeEach(() => {
	vi.useFakeTimers();
	state.pending = false;
	state.integrate.mockReset();
	preview.mockReset().mockResolvedValue(outcome(true));
	vi.stubGlobal("lite", {
		headInfo: () => head,
		workspaceTargetCommits: () => ({ commits: [], hasMore: false }),
		changesInWorktree: () => ({
			changes: [],
			ignoredChanges: [],
			modificationTimes: {},
			assignments: [],
			assignmentsError: null,
			dependencies: null,
			dependenciesError: null,
		}),
		workspaceIntegrateUpstream: preview,
	});
	client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
	client.setQueryData(headInfoQueryOptions(projectId).queryKey, head);
	client.setQueryData(workspaceTargetCommitsQueryOptions(projectId).queryKey, {
		commits: [],
		hasMore: false,
	});
	client.setQueryData(changesInWorktreeQueryOptions(projectId).queryKey, {
		changes: [],
		ignoredChanges: [],
		modificationTimes: {},
		assignments: [],
		assignmentsError: null,
		dependencies: null,
		dependenciesError: null,
	});
	container = document.createElement("div");
	document.body.append(container);
	root = createRoot(container);
});
afterEach(() => {
	act(() => root.unmount());
	client.clear();
	container.remove();
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
	vi.useRealTimers();
});
it("warns about predicted commit conflicts without applying the update", async () => {
	await render();
	await flush();
	expect(warning()).not.toBeNull();
	expect(preview).toHaveBeenCalledWith({
		projectId,
		updates: [{ kind: "rebase", selector: { type: "commit", subject: "local" } }],
		dryRun: true,
	});
	expect(state.integrate).not.toHaveBeenCalled();
	act(() =>
		Array.from(container.querySelectorAll("button"))
			.find((button) => button.textContent === "Pull latest")
			?.click(),
	);
	expect(state.integrate).toHaveBeenCalledWith({
		projectId,
		updates: [{ kind: "rebase", selector: { type: "commit", subject: "local" } }],
		dryRun: false,
	});
});
it("warns about predicted conflicts in dirty files", async () => {
	preview.mockResolvedValue(outcome(false, ["file.txt"]));
	await render();
	await flush();
	expect(warning()).not.toBeNull();
});
it("does not warn for a clean preview", async () => {
	preview.mockResolvedValue(outcome(false));
	await render();
	await flush();
	expect(warning()).toBeNull();
});
it.each([false, true])(
	"allows integration without an indicator while the preview is pending (conflicts: %s)",
	async (conflict) => {
		let resolve!: (value: WorkspaceIntegrateUpstreamOutcome) => void;
		preview.mockImplementation(
			() =>
				new Promise((done) => {
					resolve = done;
				}),
		);
		await render();
		await flush();
		expect(container.querySelector("output")).toBeNull();
		expect(warning()).toBeNull();
		expect(action().getAttribute("aria-disabled")).not.toBe("true");
		await act(async () => resolve(outcome(conflict)));
		await flush();
		expect(action().textContent).toBe("Pull latest");
		expect(action().getAttribute("aria-disabled")).not.toBe("true");
		expect(warning() !== null).toBe(conflict);
		expect(container.querySelector("output")).toBeNull();
		act(() => action().click());
		expect(state.integrate).toHaveBeenCalledOnce();
	},
);
it.each(["head", "target", "worktree"] as const)(
	"allows integration while the %s source refreshes",
	async (source) => {
		await render();
		await flush();
		const options =
			source === "head"
				? headInfoQueryOptions(projectId)
				: source === "target"
					? workspaceTargetCommitsQueryOptions(projectId)
					: changesInWorktreeQueryOptions(projectId);
		let resolve!: () => void;
		const pending = new Promise<void>((done) => {
			resolve = done;
		});
		if (source === "head") {
			vi.spyOn(window.lite, "headInfo").mockReturnValue(
				pending.then(() => assert(client.getQueryData(headInfoQueryOptions(projectId).queryKey))),
			);
		} else if (source === "target") {
			vi.spyOn(window.lite, "workspaceTargetCommits").mockReturnValue(
				pending.then(() =>
					assert(client.getQueryData(workspaceTargetCommitsQueryOptions(projectId).queryKey)),
				),
			);
		} else {
			vi.spyOn(window.lite, "changesInWorktree").mockReturnValue(
				pending.then(() =>
					assert(client.getQueryData(changesInWorktreeQueryOptions(projectId).queryKey)),
				),
			);
		}
		act(() => {
			void client.invalidateQueries({ queryKey: options.queryKey });
		});
		await flush();
		expect(container.querySelector("output")).toBeNull();
		expect(warning()).toBeNull();
		expect(action().getAttribute("aria-disabled")).not.toBe("true");
		act(() => action().click());
		expect(state.integrate).toHaveBeenCalledOnce();
		expect(preview).toHaveBeenCalledOnce();
		await act(async () => resolve());
		await flush();
		expect(action().textContent).toBe("Pull latest");
		expect(action().getAttribute("aria-disabled")).not.toBe("true");
		expect(preview).toHaveBeenCalledTimes(2);
	},
);
it("does not run previews when the base is current or an operation is pending", async () => {
	state.pending = true;
	await render();
	await flush();
	expect(preview).not.toHaveBeenCalled();
	state.pending = false;
	client.setQueryData(headInfoQueryOptions(projectId).queryKey, {
		...head,
		target: {
			remoteTrackingRef: { fullNameBytes: [], displayName: "origin/main", remoteName: "origin" },
			commitsAhead: 0,
			isCurrent: true,
		},
	});
	await render();
	await flush();
	expect(preview).not.toHaveBeenCalled();
});
it.each(["target", "head", "worktree"])(
	"rechecks after %s changes and hides the old warning while waiting",
	async (source) => {
		await render();
		await flush();
		expect(warning()).not.toBeNull();
		let resolve!: (value: WorkspaceIntegrateUpstreamOutcome) => void;
		preview.mockImplementation(
			() =>
				new Promise((r) => {
					resolve = r;
				}),
		);
		await act(async () => {
			const key =
				source === "target"
					? workspaceTargetCommitsQueryOptions(projectId).queryKey
					: source === "head"
						? headInfoQueryOptions(projectId).queryKey
						: changesInWorktreeQueryOptions(projectId).queryKey;
			client.setQueryData(key, (value) => value, { updatedAt: Date.now() + 100 });
		});
		await flush();
		expect(warning()).toBeNull();
		expect(container.querySelector("output")).toBeNull();
		act(() => action().click());
		expect(state.integrate).toHaveBeenCalledOnce();
		await act(async () => resolve(outcome(false)));
		await flush();
		expect(warning()).toBeNull();
		expect(action().textContent).toBe("Pull latest");
		expect(action().getAttribute("aria-disabled")).not.toBe("true");
		expect(preview).toHaveBeenCalledTimes(2);
	},
);
it("shows no indicator and allows integration if the preview fails", async () => {
	preview.mockRejectedValue(new Error("preview failed"));
	await render();
	await flush();
	expect(warning()).toBeNull();
	expect(container.querySelector("output")).toBeNull();
	expect(action().getAttribute("aria-disabled")).not.toBe("true");
	act(() => action().click());
	expect(state.integrate).toHaveBeenCalledOnce();
});

it("lists each commit's predicted conflicts without offering navigation", async () => {
	await render();
	await flush();
	act(() => {
		warning()?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
	});
	await flush();
	const dialog = document.querySelector('[role="dialog"]');
	expect(dialog?.textContent).toContain("feature/login");
	const commit = Array.from(dialog?.querySelectorAll("li") ?? []).find((item) =>
		item.textContent.includes("Refresh sessions"),
	);
	expect(commit?.textContent).toContain("local");
	expect(commit?.textContent).toContain("src/session.ts");
	expect(commit?.textContent).toContain("assets/logo.png");
	expect(commit?.querySelector("button, a, [tabindex]")).toBeNull();
	expect(dialog?.textContent).not.toContain("Select a commit");
	expect(state.integrate).not.toHaveBeenCalled();
});
it("lists the uncommitted files that will conflict", async () => {
	preview.mockResolvedValue(outcome(false, ["src/config.ts"]));
	await render();
	await flush();
	act(() =>
		container
			.querySelector<HTMLButtonElement>('[aria-label="Update will cause conflicts"]')
			?.click(),
	);
	await flush();
	expect(document.body.textContent).toContain("These files will conflict after the update");
	expect(document.body.textContent).toContain("src/config.ts");
});

it("keeps branches separate and omits clean commits", async () => {
	const result = outcome(true);
	const segment = result.workspaceState.headInfo.stacks[0]?.segments[0];
	if (!segment) throw new Error("Missing fixture segment");
	result.workspaceState.headInfo.stacks[0]?.segments.push({
		...segment,
		refName: { fullNameBytes: [], displayName: "fix/settings" },
		commits: [
			{ ...segment.commits[0], id: "unmapped", message: "Save preferences", hasConflicts: true },
			{ ...segment.commits[0], id: "clean", message: "Clean commit", hasConflicts: false },
		],
	} as typeof segment);
	result.commitConflicts = { ...result.commitConflicts, unmapped: ["src/settings.json"] };
	preview.mockResolvedValue(result);
	await render();
	await flush();
	act(() =>
		container
			.querySelector<HTMLButtonElement>('[aria-label="Update will cause conflicts"]')
			?.click(),
	);
	await flush();
	const groups = Array.from(document.querySelectorAll("section"));
	expect(
		groups.find((group) => group.textContent.includes("feature/login"))?.textContent,
	).toContain("Refresh sessions");
	expect(groups.find((group) => group.textContent.includes("fix/settings"))?.textContent).toContain(
		"Save preferences",
	);
	const settings = groups.find((group) => group.textContent.includes("fix/settings"));
	expect(settings?.textContent).toContain("src/settings.json");
	expect(settings?.textContent).not.toContain("src/session.ts");
	expect(document.body.textContent).not.toContain("Clean commit");
	expect(
		Array.from(document.querySelectorAll("button")).some((button) =>
			button.textContent.includes("Save preferences"),
		),
	).toBe(false);
});

it.each(["head", "target", "worktree"] as const)(
	"hides cached conflicts after a failed %s refresh and recovers on success",
	async (source) => {
		await render();
		await flush();
		expect(warning()).not.toBeNull();
		const endpoint =
			source === "head"
				? "headInfo"
				: source === "target"
					? "workspaceTargetCommits"
					: "changesInWorktree";
		const options =
			source === "head"
				? headInfoQueryOptions(projectId)
				: source === "target"
					? workspaceTargetCommitsQueryOptions(projectId)
					: changesInWorktreeQueryOptions(projectId);
		const failure = vi
			.spyOn(window.lite, endpoint)
			.mockRejectedValue(new Error("Source refresh failed"));
		await act(async () => {
			await client.invalidateQueries({ queryKey: options.queryKey });
		});
		await flush();
		expect(client.getQueryState(options.queryKey)?.status).toBe("error");
		expect(client.getQueryData(options.queryKey)).toBeDefined();
		expect(warning()).toBeNull();
		expect(container.querySelector("output")).toBeNull();
		expect(action().getAttribute("aria-disabled")).not.toBe("true");
		failure.mockRestore();
		await act(async () => {
			await client.invalidateQueries({ queryKey: options.queryKey });
		});
		await flush();
		expect(warning()).not.toBeNull();
		expect(action().textContent).toBe("Pull latest");
	},
);

it("keeps the popup closed after refreshing an open conflict preview", async () => {
	await render();
	await flush();
	act(() => {
		(warning() as HTMLButtonElement).click();
	});
	await flush();
	expect(document.querySelector('[role="dialog"]')).not.toBeNull();
	let resolve!: (value: WorkspaceIntegrateUpstreamOutcome) => void;
	preview.mockImplementation(
		() =>
			new Promise((r) => {
				resolve = r;
			}),
	);
	await act(async () => {
		client.setQueryData(headInfoQueryOptions(projectId).queryKey, (value) => value, {
			updatedAt: Date.now() + 100,
		});
	});
	await flush();
	expect(warning()).toBeNull();
	expect(document.querySelector('[role="dialog"]')).toBeNull();
	await act(async () => resolve(outcome(true)));
	await flush();
	expect(warning()).not.toBeNull();
	expect(document.querySelector('[role="dialog"]')).toBeNull();
	act(() => {
		(warning() as HTMLButtonElement).click();
	});
	await flush();
	expect(document.querySelector('[role="dialog"]')).not.toBeNull();
	act(() => {
		document.querySelector<HTMLButtonElement>('[aria-label="Close conflict details"]')?.click();
	});
	await flush();
	expect(document.querySelector('[role="dialog"]')).toBeNull();
});

it.each(["current", "preview"] as const)(
	"attributes anonymous %s segments to the next named branch below",
	async (source) => {
		const current = structuredClone(head);
		const result = outcome(true);
		if (source === "preview") result.workspaceState.replacedCommits = {};
		const stack = (source === "current" ? current : result.workspaceState.headInfo).stacks[0];
		const segment = stack?.segments[0];
		if (!stack || !segment) throw new Error("Missing fixture segment");
		stack.segments = [
			{ ...segment, refName: { fullNameBytes: [], displayName: "unrelated/above" }, commits: [] },
			{ ...segment, refName: null },
			{ ...segment, refName: null, commits: [] },
			{ ...segment, refName: { fullNameBytes: [], displayName: "feature/login" }, commits: [] },
		];
		client.setQueryData(headInfoQueryOptions(projectId).queryKey, current);
		preview.mockResolvedValue(result);
		await render();
		await flush();
		act(() => {
			(warning() as HTMLButtonElement).click();
		});
		await flush();
		const dialog = document.querySelector('[role="dialog"]');
		expect(dialog?.textContent).toContain("feature/login");
		expect(dialog?.textContent).toContain("Refresh sessions");
		expect(dialog?.textContent).not.toContain("Unnamed branch");
		expect(dialog?.textContent).not.toContain("unrelated/above");
	},
);

it("combines rapid revisions into one preview of the latest state", async () => {
	await render();
	for (const id of ["first", "second", "latest"]) {
		await flush(50);
		await reviseHead(id);
	}
	expect(preview).not.toHaveBeenCalled();
	await flush();
	expect(preview).toHaveBeenCalledOnce();
	expect(preview).toHaveBeenLastCalledWith({
		projectId,
		updates: [{ kind: "rebase", selector: { type: "commit", subject: "latest" } }],
		dryRun: true,
	});
});

it.each([false, true])(
	"runs only the newest waiting preview after the active call settles (failure: %s)",
	async (failed) => {
		const active = Promise.withResolvers<WorkspaceIntegrateUpstreamOutcome>();
		preview.mockImplementationOnce(() => active.promise);
		try {
			await render();
			await flush();
			expect(preview).toHaveBeenCalledOnce();
			for (const id of ["first", "second", "latest"]) {
				await reviseHead(id);
				await flush();
			}
			expect(preview).toHaveBeenCalledOnce();
			expect(warning()).toBeNull();
			await act(async () => {
				if (failed) active.reject(new Error("Old preview failed"));
				else active.resolve(outcome(false));
			});
			await flush();
			expect(preview).toHaveBeenCalledTimes(2);
			expect(preview).toHaveBeenLastCalledWith({
				projectId,
				updates: [{ kind: "rebase", selector: { type: "commit", subject: "latest" } }],
				dryRun: true,
			});
			expect(warning()).not.toBeNull();
		} finally {
			await act(async () => active.resolve(outcome(false)));
		}
	},
);

it("drops a waiting preview when a source refresh starts", async () => {
	const source = Promise.withResolvers<RefInfo>();
	vi.spyOn(window.lite, "headInfo").mockReturnValue(source.promise);
	await render();
	await flush(50);
	act(() => {
		void client.invalidateQueries({ queryKey: headInfoQueryOptions(projectId).queryKey });
	});
	await flush();
	expect(preview).not.toHaveBeenCalled();
	await act(async () => source.resolve(head));
	await flush();
	expect(preview).toHaveBeenCalledOnce();
});

it.each(["unmount", "operation", "integrate"] as const)(
	"drops a waiting preview on %s",
	async (reason) => {
		await render();
		await flush(50);
		if (reason === "unmount") {
			act(() => root.render(null));
		} else if (reason === "operation") {
			state.pending = true;
			await render();
		} else {
			act(() => action().click());
			expect(state.integrate).toHaveBeenCalledOnce();
		}
		await flush();
		expect(preview).not.toHaveBeenCalled();
	},
);

it("drops queued previews when Integrate is clicked during an active check", async () => {
	const active = Promise.withResolvers<WorkspaceIntegrateUpstreamOutcome>();
	preview.mockImplementationOnce(() => active.promise);
	try {
		await render();
		await flush();
		await reviseHead("latest");
		await flush();
		act(() => action().click());
		expect(state.integrate).toHaveBeenCalledOnce();
		await act(async () => active.resolve(outcome(false)));
		await flush();
		expect(preview).toHaveBeenCalledOnce();
	} finally {
		await act(async () => active.resolve(outcome(false)));
	}
});

it("keeps active checks across project switches without blocking other projects", async () => {
	const active = Promise.withResolvers<WorkspaceIntegrateUpstreamOutcome>();
	preview.mockImplementationOnce(() => active.promise);
	try {
		await render();
		await flush();
		expect(preview).toHaveBeenCalledOnce();
		await render("other-project");
		await flush();
		await flush();
		expect(preview).toHaveBeenCalledTimes(2);
		expect(preview).toHaveBeenLastCalledWith(
			expect.objectContaining({ projectId: "other-project" }),
		);
		await render();
		await flush();
		expect(preview).toHaveBeenCalledTimes(2);
		await act(async () => active.resolve(outcome(false)));
		await flush();
		expect(preview).toHaveBeenCalledTimes(3);
		expect(preview).toHaveBeenLastCalledWith(expect.objectContaining({ projectId }));
	} finally {
		await act(async () => active.resolve(outcome(false)));
	}
});

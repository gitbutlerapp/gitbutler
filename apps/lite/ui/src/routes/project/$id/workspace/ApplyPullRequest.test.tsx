/** @vitest-environment jsdom */

import type { ApplyOutcome, ForgeInfo, ForgeReview } from "@gitbutler/but-sdk";
import { Toolbar } from "@base-ui/react";
import type * as BaseUI from "@base-ui/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setCursor, setPage } from "#ui/use-cursor.ts";
import { ApplyPullRequest } from "./ApplyPullRequest.tsx";

const toastAdd = vi.hoisted(() =>
	vi.fn<(toast: { type: string; title: string; description?: string }) => void>(),
);

vi.mock("#ui/store.ts", () => ({ useAppDispatch: () => vi.fn() }));
vi.mock("#ui/use-cursor.ts", () => ({ setCursor: vi.fn(), setPage: vi.fn() }));
vi.mock("@base-ui/react", async (importOriginal) => {
	const original = await importOriginal<typeof BaseUI>();
	return { ...original, Toast: { ...original.Toast, useToastManager: () => ({ add: toastAdd }) } };
});

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const forgeInfo = (name: ForgeInfo["name"], prService = true): ForgeInfo => ({
	name,
	baseUrl: name === "gitlab" ? "https://gitlab.com/acme/repo" : "https://github.com/acme/repo",
	commitUrlPath: "/commit/",
	prUrlPath: "/pull/",
	unit: { symbol: "#", name: "pull request", abbr: "PR" },
	posthogLabel: "",
	capabilities: {
		prService,
		checks: false,
		repoInfo: false,
		listService: false,
		reviewComments: false,
		reviewManagement: false,
	},
});

const review = (number: number) =>
	({ number, title: `Fork PR ${number}`, sourceBranch: "feature", unitSymbol: "#" }) as ForgeReview;

const outcome = (status: ApplyOutcome["status"], refs: Array<string> = []): ApplyOutcome => ({
	status,
	workspaceChanged: status === "applied",
	appliedBranches: refs.map((full) => ({ full })),
	workspaceRefCreated: false,
	conflictingStacks:
		status === "conflictAborted" ? [{ refName: { full: "refs/heads/a" }, shortName: "a" }] : [],
});

const deferred = <T,>() => {
	let resolve!: (value: T) => void;
	let reject!: (error: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
};

const lite = {
	forgeInfo: vi.fn(),
	listKnownGithubAccounts: vi.fn(),
	listKnownGitlabAccounts: vi.fn(),
	getReview: vi.fn(),
	reviewApply: vi.fn(),
};

let client: QueryClient;
let root: Root;

// React Query notifies its observers on a timeout, and chained queries and
// invalidations each take a few.
const flush = () =>
	act(async () => {
		for (let tick = 0; tick < 10; tick++) await new Promise((resolve) => setTimeout(resolve, 0));
	});

const render = async () => {
	const container = document.createElement("div");
	document.body.append(container);
	root = createRoot(container);
	act(() =>
		root.render(
			<QueryClientProvider client={client}>
				<Toolbar.Root>
					<ApplyPullRequest projectId="project" />
				</Toolbar.Root>
			</QueryClientProvider>,
		),
	);
	await flush();
};

const action = () =>
	document.querySelector<HTMLButtonElement>('button[aria-label="Apply pull request"]');
const dialog = () => document.querySelector('[role="dialog"]');
const input = () => document.querySelector<HTMLInputElement>('[role="dialog"] input');
const submitButton = () =>
	document.querySelector<HTMLButtonElement>('[role="dialog"] button[type="submit"]');
const cancelButton = () =>
	[...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')].find(
		(button) => button.textContent === "Cancel",
	);

const open = async () => {
	act(() => action()?.click());
	// The modal holds its first frame closed so that it can animate open.
	await act(() => new Promise((resolve) => requestAnimationFrame(resolve)));
	await flush();
};

const type = async (value: string) => {
	const element = input();
	if (!element) throw new Error("No input");
	act(() => {
		Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(element, value);
		element.dispatchEvent(new Event("input", { bubbles: true }));
	});
	await flush();
};

const submit = async () => {
	act(() => submitButton()?.click());
	await flush();
};

const pressEnter = async () => {
	act(() => {
		input()?.form?.requestSubmit();
	});
	await flush();
};

const preview = async (value: string, number: number) => {
	lite.getReview.mockResolvedValueOnce(review(number));
	await type(value);
	await submit();
};

beforeEach(() => {
	vi.clearAllMocks();
	vi.stubGlobal("lite", lite);
	lite.forgeInfo.mockResolvedValue(forgeInfo("github"));
	lite.listKnownGithubAccounts.mockResolvedValue([
		{ type: "patUsername", info: { username: "me" } },
	]);
	lite.listKnownGitlabAccounts.mockResolvedValue([
		{ type: "patUsername", info: { username: "me" } },
	]);
	lite.getReview.mockResolvedValue(review(42));
	client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
});

afterEach(() => {
	act(() => root.unmount());
	document.body.replaceChildren();
	client.clear();
	vi.unstubAllGlobals();
});

describe("availability", () => {
	it("is offered for a GitHub project with an account", async () => {
		await render();
		expect(action()).not.toBeNull();
	});

	it("is hidden without a GitHub account", async () => {
		lite.listKnownGithubAccounts.mockResolvedValue([]);
		await render();
		expect(action()).toBeNull();
	});

	it("is hidden for other forges", async () => {
		lite.forgeInfo.mockResolvedValue(forgeInfo("gitlab"));
		await render();
		expect(action()).toBeNull();
	});

	it("is hidden without pull request support", async () => {
		lite.forgeInfo.mockResolvedValue(forgeInfo("github", false));
		await render();
		expect(action()).toBeNull();
	});
});

describe("input", () => {
	it.each([
		"",
		"   ",
		"+42",
		"-42",
		"0",
		"042",
		"4.2",
		"42.0",
		"4e2",
		"abc",
		"42a",
		"#42",
		"https://github.com/acme/repo/pull/42",
		"9007199254740992",
	])("rejects %j without fetching or applying", async (value) => {
		await render();
		await open();
		await type(value);
		await submit();
		expect(dialog()?.textContent).toContain("Enter the number only, like 42.");
		expect(lite.getReview).not.toHaveBeenCalled();
		expect(lite.reviewApply).not.toHaveBeenCalled();
	});

	it("previews a trimmed number", async () => {
		await render();
		await open();
		await preview("  9007199254740991 ", 9007199254740991);
		expect(lite.getReview).toHaveBeenCalledWith({
			projectId: "project",
			reviewId: 9007199254740991,
		});
		expect(dialog()?.textContent).toContain("Fork PR 9007199254740991");
		expect(submitButton()?.textContent).toBe("Apply to workspace");
	});
});

describe("preview", () => {
	it("fetches once when Continue and Enter race", async () => {
		const pending = deferred<ForgeReview>();
		lite.getReview.mockReturnValue(pending.promise);
		await render();
		await open();
		await type("42");
		await submit();
		await pressEnter();
		await submit();
		expect(lite.getReview).toHaveBeenCalledTimes(1);
		expect(submitButton()?.disabled).toBe(true);
		pending.resolve(review(42));
		await flush();
		expect(submitButton()?.textContent).toBe("Apply to workspace");
	});

	it("drops a preview that resolves after an edit", async () => {
		const pending = deferred<ForgeReview>();
		lite.getReview.mockReturnValue(pending.promise);
		await render();
		await open();
		await type("42");
		await submit();
		await type("43");
		pending.resolve(review(42));
		await flush();
		expect(dialog()?.textContent).not.toContain("Fork PR 42");
		expect(submitButton()?.textContent).toBe("Continue");
	});

	it("drops a shown preview on edit", async () => {
		await render();
		await open();
		await preview("42", 42);
		await type("421");
		expect(dialog()?.textContent).not.toContain("Fork PR 42");
		expect(submitButton()?.textContent).toBe("Continue");
		expect(lite.reviewApply).not.toHaveBeenCalled();
	});

	it("reports a failed preview and retries it once on Continue", async () => {
		lite.getReview.mockRejectedValueOnce(new Error("Not Found"));
		await render();
		await open();
		await type("42");
		await submit();
		expect(dialog()?.textContent).toContain("Could not load the pull request.");
		expect(submitButton()?.textContent).toBe("Continue");

		lite.getReview.mockResolvedValueOnce(review(42));
		await submit();
		expect(lite.getReview).toHaveBeenCalledTimes(2);
		expect(submitButton()?.textContent).toBe("Apply to workspace");
		expect(lite.reviewApply).not.toHaveBeenCalled();
	});
});

describe("apply", () => {
	it("applies the previewed number once and cannot close meanwhile", async () => {
		const pending = deferred<ApplyOutcome>();
		lite.reviewApply.mockReturnValue(pending.promise);
		await render();
		await open();
		await preview(" 42", 42);
		await submit();
		await pressEnter();
		await submit();
		expect(lite.reviewApply).toHaveBeenCalledTimes(1);
		expect(lite.reviewApply).toHaveBeenCalledWith({ projectId: "project", reviewId: 42 });
		expect(submitButton()?.disabled).toBe(true);
		expect(cancelButton()?.disabled).toBe(true);
		expect(input()?.disabled).toBe(true);

		act(() => {
			document.activeElement?.dispatchEvent(
				new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
			);
		});
		await flush();
		expect(dialog()).not.toBeNull();

		pending.resolve(outcome("conflictAborted"));
		await flush();
	});

	it("refreshes the caches, then follows the pull request's branch", async () => {
		const invalidate = vi.spyOn(client, "invalidateQueries");
		lite.reviewApply.mockResolvedValue(
			outcome("applied", ["refs/heads/checked-out", "refs/heads/feature"]),
		);
		await render();
		await open();
		await preview("42", 42);
		await submit();

		expect(dialog()).toBeNull();
		expect(setPage).toHaveBeenCalledWith("workspace");
		expect(setCursor).toHaveBeenCalledTimes(1);
		expect(setCursor).toHaveBeenCalledWith("applied", {
			_tag: "Branch",
			branchRef: [...new TextEncoder().encode("refs/heads/feature")],
		});
		expect(invalidate).toHaveBeenCalled();
		expect(invalidate.mock.invocationCallOrder.at(-1)).toBeLessThan(
			vi.mocked(setPage).mock.invocationCallOrder[0] ?? 0,
		);
	});

	it("closes without following an already applied pull request", async () => {
		lite.reviewApply.mockResolvedValue(outcome("alreadyApplied"));
		await render();
		await open();
		await preview("42", 42);
		await submit();

		expect(dialog()).toBeNull();
		expect(toastAdd).toHaveBeenCalledWith(
			expect.objectContaining({ type: "info", title: "Pull request already in workspace" }),
		);
		expect(setPage).not.toHaveBeenCalled();
		expect(setCursor).not.toHaveBeenCalled();
	});

	it("stays open on conflicts and names them", async () => {
		lite.reviewApply.mockResolvedValue(outcome("conflictAborted"));
		await render();
		await open();
		await preview("42", 42);
		await submit();

		expect(dialog()).not.toBeNull();
		expect(toastAdd.mock.lastCall?.[0]).toMatchObject({
			type: "error",
			description: "It conflicts with existing stacks in the workspace: a",
		});
		expect(setPage).not.toHaveBeenCalled();
		expect(setCursor).not.toHaveBeenCalled();
	});

	it("stays open when an applied outcome names no branch", async () => {
		lite.reviewApply.mockResolvedValue(outcome("applied"));
		await render();
		await open();
		await preview("42", 42);
		await submit();

		expect(dialog()).not.toBeNull();
		expect(toastAdd).toHaveBeenCalledWith(expect.objectContaining({ type: "error" }));
		expect(setPage).not.toHaveBeenCalled();
		expect(setCursor).not.toHaveBeenCalled();
	});

	it("stays open when applying fails", async () => {
		lite.reviewApply.mockRejectedValue(new Error("fetch failed"));
		await render();
		await open();
		await preview("42", 42);
		await submit();

		expect(dialog()).not.toBeNull();
		expect(submitButton()?.disabled).toBe(false);
		expect(setPage).not.toHaveBeenCalled();
		expect(setCursor).not.toHaveBeenCalled();
	});
});

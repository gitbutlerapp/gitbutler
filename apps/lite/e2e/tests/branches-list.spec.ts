import { enabled, shoot } from "../screenshot-helpers.ts";
import type { LiteElectronApi } from "../../electron/src/ipc.ts";
import type { ListedStack } from "@gitbutler/but-sdk";
import { expect, test } from "../test.ts";

test.describe("branches list", () => {
	test.use({ scenario: "project-with-many-unapplied-branches.sh" });

	// Each unfolded branch runs a commit virtualizer on the borrowed list
	// scroller, so with the selected branch scrolled away from, they used to
	// stamp the offset they last remembered back over the scroll the user made.
	test("scrolls to the last branch while the first one is selected", async ({ appWindow }) => {
		// Seeding a project this size and then unfolding and driving it by hand
		// costs more than the default budget allows for on a loaded CI runner.
		test.slow();

		await appWindow
			.getByRole("group", { name: "Pages" })
			.getByRole("button", { name: "Branches" })
			.click();

		const branches = appWindow.getByRole("treeitem", { name: /^branch-\d+$/ });
		await expect(branches.first()).toBeVisible();

		// `unfold` asks every branch the list currently has mounted to show its
		// commits, which is what puts a commit virtualizer on the shared scroller.
		// It rides along with the scroll reading because both need the same
		// element and each round trip costs the same as the whole batch.
		const scroll = (unfold = false): Promise<{ offset: number; end: number }> =>
			appWindow.evaluate((unfoldMounted) => {
				// Anchored on a branch row: the hidden sidebar tabs are still in the
				// document, and they look just like this list from the outside.
				const row = document.querySelector('[role="treeitem"][aria-label^="branch-"]');
				const tree = row?.closest('[role="tree"]');
				const scroller = tree?.parentElement;
				if (!tree || !scroller) throw new Error("Branches list has no scroller");

				if (unfoldMounted) {
					for (const toggle of tree.querySelectorAll<HTMLElement>(
						'button[aria-label="Unfold commits"]',
					))
						toggle.click();
				}

				return {
					offset: Math.round(scroller.scrollTop),
					end: Math.round(scroller.scrollHeight - scroller.clientHeight),
				};
			}, unfold);

		const start = await branches.first().boundingBox();
		if (start === null) throw new Error("Branch row has no bounding box");
		await appWindow.mouse.move(start.x + start.width / 2, start.y + start.height / 2);

		// The list virtualises, so unfolding what is mounted stages more rows to
		// unfold: walk down it until the last of them is showing its commits.
		for (let pass = 0; pass < 10; pass++) {
			const { offset, end } = await scroll(true);
			if (offset >= end) break;
			await appWindow.mouse.wheel(0, 600);
		}

		await appWindow.evaluate(() => {
			const row = document.querySelector('[role="treeitem"][aria-label^="branch-"]');
			row?.closest('[role="tree"]')?.parentElement?.scrollTo({ top: 0 });
		});

		await expect(branches.first()).toBeVisible();
		// Pin the branch by name: which one the list puts first is its own
		// business, and every locator below has to keep meaning that same one.
		const firstName = await branches.first().getAttribute("aria-label");
		if (firstName === null) throw new Error("Branch row has no name");

		const first = appWindow.getByRole("treeitem", { name: firstName, exact: true });
		// The tree item spans the branch row and the commits under it, so aim at
		// the name rather than the item's middle, which is a commit row.
		await first.getByTitle(firstName, { exact: true }).click();
		await expect(first).toHaveAttribute("aria-selected", "true");

		// The branches have to outgrow their panel for any of this to mean anything.
		expect((await scroll()).end).toBeGreaterThan(0);

		const box = await first.boundingBox();
		if (box === null) throw new Error("Branch row has no bounding box");
		await appWindow.mouse.move(box.x + box.width / 2, box.y + box.height / 2);

		// Enough wheel to cross the whole list several times over; the scroller
		// stops at its end. Few, large steps rather than many small ones: each one
		// is a round trip, and a stuck list stays stuck however finely it is asked
		// to move.
		for (let step = 0; step < 16; step++) await appWindow.mouse.wheel(0, 400);

		await expect
			.poll(async () => {
				const { offset, end } = await scroll();
				return offset >= end;
			})
			.toBe(true);
	});
});

test.describe("recent branch reviews", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("shows review metadata and filters by labels, authors, and review numbers", async ({
		appWindow,
		electronApp,
	}) => {
		const stacks = await appWindow.evaluate(async () => {
			const projectId = location.pathname.split("/")[2];
			if (projectId === undefined) throw new Error("No project in the URL");
			return (window as unknown as { lite: LiteElectronApi }).lite.branchList(projectId);
		});
		const enriched: Array<ListedStack> = stacks.map((stack) => ({
			...stack,
			branches: stack.branches.map((branch) =>
				branch.displayName === "branch1"
					? {
							...branch,
							reviewStatus: "open",
							review: {
								number: 42,
								title: "Speed up branch listing in large repositories",
								htmlUrl: "https://example.com/pull/42",
								unitSymbol: "#",
								createdAt: "2026-09-01T12:00:00Z",
								author: { login: "octocat", name: null },
								labels: [
									{ name: "performance", color: "0e8a16", description: "Performance improvements" },
									{ name: "@gitbutler/lite", color: "#ffffff", description: null },
									{ name: "needs review", color: null, description: null },
								],
							},
						}
					: branch.displayName === "branch2"
						? {
								...branch,
								reviewStatus: "draft",
								review: {
									number: 43,
									title: "A draft without optional metadata",
									htmlUrl: "https://example.com/pull/43",
									unitSymbol: "!",
									labels: [],
									author: null,
									createdAt: null,
								},
							}
						: branch,
			),
		}));
		await electronApp.evaluate(({ ipcMain }, stacks) => {
			ipcMain.removeHandler("branchList");
			ipcMain.handle("branchList", () => stacks);
			ipcMain.removeHandler("openInWebBrowser");
			ipcMain.handle("openInWebBrowser", (_event, url: string) => {
				(globalThis as { openedReviewUrl?: string }).openedReviewUrl = url;
			});
		}, enriched);
		await appWindow.reload();
		await appWindow
			.getByRole("group", { name: "Pages" })
			.getByRole("button", { name: "Branches", exact: true })
			.click();
		const branch = appWindow.getByRole("treeitem", { name: "branch1", exact: true });
		await expect(branch).toHaveAccessibleDescription(
			/Speed up branch listing in large repositories.*octocat.*performance/,
		);
		await expect(
			branch.getByText("Speed up branch listing in large repositories", { exact: true }),
		).toBeVisible();
		await expect(branch.getByText("performance", { exact: true })).toBeVisible();
		await expect(branch.getByText("needs review", { exact: true })).toHaveCSS(
			"background-color",
			"rgba(0, 0, 0, 0)",
		);
		await expect(branch.getByText("octocat", { exact: true })).toBeVisible();
		await expect(branch.getByText("2 commits", { exact: true })).toBeVisible();
		await expect(
			appWindow.getByRole("treeitem", { name: "Draft group", exact: true }),
		).toBeVisible();
		await expect(
			appWindow
				.getByRole("treeitem", { name: "branch3", exact: true })
				.getByTitle("branch3", { exact: true }),
		).toBeVisible();

		const sidebar = appWindow.locator("#sidebar-panel");
		await appWindow
			.getByRole("treeitem", { name: "branch3", exact: true })
			.getByTitle("branch3", { exact: true })
			.click();
		await appWindow.mouse.move(800, 40);
		const heading = branch.getByTitle("Speed up branch listing in large repositories", {
			exact: true,
		});
		const label = branch.getByText("performance", { exact: true });
		const beforeHover = {
			row: await branch.boundingBox(),
			heading: await heading.boundingBox(),
			label: await label.boundingBox(),
		};
		await expect(branch.getByRole("button", { name: "Branch menu" })).toBeHidden();
		await heading.hover();
		await expect(branch.getByRole("button", { name: "Branch menu" })).toBeVisible();
		expect({
			row: await branch.boundingBox(),
			heading: await heading.boundingBox(),
			label: await label.boundingBox(),
		}).toEqual(beforeHover);
		expect(await sidebar.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);

		await branch.getByRole("button", { name: "Open #42 in browser" }).click();
		expect(
			await electronApp.evaluate(
				() => (globalThis as { openedReviewUrl?: string }).openedReviewUrl,
			),
		).toBe("https://example.com/pull/42");
		await branch.getByTitle("branch1", { exact: true }).click();
		await expect(branch).toHaveAttribute("aria-selected", "true");
		await expect
			.poll(() =>
				branch.evaluate((element) => {
					const row = element.matches('[class*="containerSelected"]')
						? element
						: element.querySelector('[class*="containerSelected"]');
					if (!row) return false;
					const probe = document.createElement("span");
					probe.style.backgroundColor = "color-mix(in srgb, var(--fill-pop-bg) 10%, transparent)";
					row.append(probe);
					const matches =
						getComputedStyle(row).backgroundColor === getComputedStyle(probe).backgroundColor;
					probe.remove();
					return matches;
				}),
			)
			.toBe(true);

		await branch.getByRole("button", { name: "Unfold commits" }).click();
		await expect(branch).toHaveAttribute("aria-expanded", "true");
		await expect(branch.getByRole("treeitem", { name: "branch1: second commit" })).toBeVisible();
		await branch.getByRole("button", { name: "Fold commits" }).click();

		await appWindow.getByRole("button", { name: "Filter branches", exact: true }).click();
		const filter = appWindow.getByRole("textbox", { name: "Filter branches" });
		for (const query of ["performance", "octocat", "#42"]) {
			await filter.fill(query);
			await expect(branch).toBeVisible();
			await expect(appWindow.getByRole("treeitem", { name: "branch2", exact: true })).toHaveCount(
				0,
			);
		}
		await filter.fill("!43");
		await expect(appWindow.getByRole("treeitem", { name: "branch2", exact: true })).toBeVisible();
		await expect(branch).toHaveCount(0);
		await filter.fill("#999");
		await expect(appWindow.getByText("No branches match", { exact: true })).toBeVisible();
		await filter.press("Escape");
		await expect(branch).toBeVisible();
	});
});

test.describe("branch state groups", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });
	test("keeps open and draft work visible above 27 collapsed merged branches", async ({
		appWindow,
		electronApp,
	}) => {
		await appWindow.setViewportSize({ width: 1440, height: 900 });
		const source = await appWindow.evaluate(async () =>
			(window as unknown as { lite: LiteElectronApi }).lite.branchList(
				location.pathname.split("/")[2] ?? "",
			),
		);
		const template = source.flatMap((stack) => stack.branches)[0];
		if (!template) throw new Error("Expected a seeded branch");
		const stacks: Array<ListedStack> = Array.from({ length: 34 }, (_, index) => {
			const state = index < 27 ? "merged" : index < 31 ? "open" : index < 33 ? "draft" : null;
			return {
				status: "standalone",
				updatedAtMs: null,
				branches: [
					{
						...template,
						displayName: `review-pass-${index}`,
						refName: { full: `refs/heads/review-pass-${index}` },
						commitCount: 1,
						reviewStatus: state,
						review:
							state === null
								? null
								: {
										title: `Review ${index}`,
										number: index + 1,
										htmlUrl: `https://example.com/pull/${index + 1}`,
										unitSymbol: "#",
										createdAt: null,
										author: { login: "octocat", name: null },
										labels: [],
									},
					},
				],
			};
		});
		await electronApp.evaluate(({ ipcMain }, stacks) => {
			ipcMain.removeHandler("branchList");
			ipcMain.handle("branchList", () => stacks);
		}, stacks);
		await appWindow.reload();
		await appWindow
			.getByRole("group", { name: "Pages" })
			.getByRole("button", { name: "Branches", exact: true })
			.click();
		for (let index = 27; index < 33; index++) {
			await expect(
				appWindow.getByRole("treeitem", { name: `review-pass-${index}`, exact: true }),
			).toBeInViewport();
		}
		const merged = appWindow.getByRole("treeitem", { name: "Merged group", exact: true });
		await expect(merged).toHaveAttribute("aria-expanded", "false");
		await expect(merged.getByText("27", { exact: true })).toBeVisible();
		await expect(
			appWindow.getByRole("treeitem", { name: "review-pass-0", exact: true }),
		).toHaveCount(0);
		if (enabled) await shoot(appWindow, "branch-state-groups", "#sidebar-panel");
		await merged.getByRole("button").click();
		await expect(merged).toHaveAttribute("aria-expanded", "true");
		await expect(
			appWindow.getByRole("treeitem", { name: "review-pass-0", exact: true }),
		).toBeVisible();
		await appWindow.getByRole("button", { name: "Author", exact: true }).click();
		await expect(
			appWindow.getByRole("treeitem", { name: "octocat group", exact: true }),
		).toBeVisible();
	});
});

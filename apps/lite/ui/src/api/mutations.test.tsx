/** @vitest-environment jsdom */

import {
	useRequestReview,
	useWithdrawReviewRequest,
	useWorkspaceIntegrateUpstream,
} from "#ui/api/mutations.ts";
import {
	getReviewQueryOptions,
	reviewerCandidatesQueryOptions,
	workspaceTargetCommitsQueryOptions,
} from "#ui/api/queries.ts";
import type { ForgeReview, ForgeReviewUser } from "@gitbutler/but-sdk";
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { act, type FC } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from "vitest";

vi.mock("#ui/store.ts", () => ({ useAppDispatch: () => vi.fn() }));
vi.mock("#ui/use-cursor.ts", () => ({}));
vi.mock("@base-ui/react", () => ({ Toast: { useToastManager: () => ({ add: vi.fn() }) } }));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const baseKey = workspaceTargetCommitsQueryOptions("project").queryKey;
const otherKey = workspaceTargetCommitsQueryOptions("other-project").queryKey;
const original = { commits: [], hasMore: true };
const updated = { commits: [], hasMore: false };

describe("useWorkspaceIntegrateUpstream", () => {
	let client: QueryClient;
	let root: Root;
	let integrate: ReturnType<typeof useWorkspaceIntegrateUpstream>["mutateAsync"];
	const workspaceIntegrateUpstream = vi.fn();

	beforeEach(() => {
		vi.stubGlobal("lite", { workspaceIntegrateUpstream });
		client = new QueryClient();
		client.setQueryData(baseKey, original);
		client.setQueryData(otherKey, original);
		const container = document.createElement("div");
		root = createRoot(container);
		const Probe = () => {
			const mutation = useWorkspaceIntegrateUpstream();
			return (
				<button
					type="button"
					onClick={() => {
						integrate = mutation.mutateAsync;
					}}
				>
					Connect
				</button>
			);
		};
		act(() =>
			root.render(
				<QueryClientProvider client={client}>
					<Probe />
				</QueryClientProvider>,
			),
		);
		act(() => container.querySelector("button")?.click());
	});

	afterEach(() => {
		act(() => root.unmount());
		client.clear();
		vi.unstubAllGlobals();
	});

	const run = async (targetCommits: typeof updated | null | undefined, dryRun = false) => {
		workspaceIntegrateUpstream.mockResolvedValue({ workspace: null, targetCommits });
		await act(async () => {
			await integrate({ projectId: "project", updates: [], dryRun });
		});
	};

	it("seeds the listing without invalidating it", async () => {
		await run(updated);
		expect(client.getQueryData(baseKey)).toEqual(updated);
		expect(client.getQueryState(baseKey)?.isInvalidated).toBe(false);
		expect(client.getQueryState(otherKey)?.isInvalidated).toBe(false);
	});

	it.each([null, undefined])("invalidates the listing when it is %s", async (listing) => {
		await run(listing);
		expect(client.getQueryData(baseKey)).toEqual(original);
		expect(client.getQueryState(baseKey)?.isInvalidated).toBe(true);
		expect(client.getQueryState(otherKey)?.isInvalidated).toBe(false);
	});

	it("leaves cached listings untouched for a dry run", async () => {
		await run(updated, true);
		expect(client.getQueryData(baseKey)).toEqual(original);
		expect(client.getQueryState(baseKey)?.isInvalidated).toBe(false);
	});
});

const projectId = "project-id";
const reviewId = 7;
const reviewKey = getReviewQueryOptions({ projectId, reviewId }).queryKey;

const user = (id: number, login: string): ForgeReviewUser => ({
	id,
	login,
	name: null,
	email: null,
	avatarUrl: null,
	isBot: false,
});
const bob = user(1, "bob");
const alice = user(2, "alice");
const reviewWith = (reviewers: Array<ForgeReviewUser>) =>
	({ number: reviewId, reviewers }) as ForgeReview;

/** Shows the cached review's reviewers as `login:id`, with a button per mutation. */
const Probe: FC<{ request: Array<string>; withdraw: Array<string> }> = (props) => {
	const { data: review } = useQuery(getReviewQueryOptions({ projectId, reviewId }));
	const { mutate: request } = useRequestReview(projectId);
	const { mutate: withdraw } = useWithdrawReviewRequest(projectId);
	return (
		<>
			<output>{review?.reviewers.map((each) => `${each.login}:${each.id}`).join(" ")}</output>
			<button
				id="request"
				onClick={() => request({ projectId, reviewId, logins: props.request })}
				type="button"
			>
				request
			</button>
			<button
				id="withdraw"
				onClick={() => withdraw({ projectId, reviewId, logins: props.withdraw })}
				type="button"
			>
				withdraw
			</button>
		</>
	);
};

describe("reviewer request mutations", () => {
	let container: HTMLDivElement;
	let queryClient: QueryClient;
	let root: Root;
	let lite: { requestReview: Mock; withdrawReviewRequest: Mock; getReview: Mock };

	beforeEach(() => {
		lite = { requestReview: vi.fn(), withdrawReviewRequest: vi.fn(), getReview: vi.fn() };
		vi.stubGlobal("lite", lite);
		container = document.createElement("div");
		document.body.append(container);
		queryClient = new QueryClient({
			defaultOptions: { queries: { retry: false, staleTime: Number.POSITIVE_INFINITY } },
		});
		queryClient.setQueryData(reviewKey, reviewWith([bob]));
		queryClient.setQueryData(reviewerCandidatesQueryOptions(projectId).queryKey, [bob, alice]);
		root = createRoot(container);
	});

	afterEach(() => {
		act(() => root.unmount());
		queryClient.clear();
		container.remove();
		vi.unstubAllGlobals();
	});

	const reviewers = () => container.querySelector("output")?.textContent;
	const render = (props: { request?: Array<string>; withdraw?: Array<string> } = {}) =>
		act(() =>
			root.render(
				<QueryClientProvider client={queryClient}>
					<Probe request={props.request ?? []} withdraw={props.withdraw ?? []} />
				</QueryClientProvider>,
			),
		);
	const click = (id: string) => container.querySelector<HTMLButtonElement>(`#${id}`)?.click();

	it("shows a requested reviewer before the forge answers", async () => {
		lite.requestReview.mockReturnValue(new Promise(() => {}));
		render({ request: ["alice"] });
		expect(reviewers()).toBe("bob:1");

		await act(async () => {
			click("request");
			await vi.waitFor(() => expect(reviewers()).toBe("bob:1 alice:2"));
		});
		expect(lite.requestReview.mock.calls[0]?.[0]).toEqual({
			projectId,
			reviewId,
			logins: ["alice"],
		});
	});

	it("adds a login once however often it is named", async () => {
		lite.requestReview.mockReturnValue(new Promise(() => {}));
		render({ request: ["alice", "alice", "bob"] });

		await act(async () => {
			click("request");
			await vi.waitFor(() => expect(reviewers()).toBe("bob:1 alice:2"));
		});
	});

	it("stands in for a login the candidate listing does not know", async () => {
		lite.requestReview.mockReturnValue(new Promise(() => {}));
		render({ request: ["carol"] });

		await act(async () => {
			click("request");
			await vi.waitFor(() => expect(reviewers()).toMatch(/^bob:1 carol:-\d+$/));
		});
	});

	it("drops the reviewer again and refetches when the forge refuses", async () => {
		lite.requestReview.mockRejectedValue(new Error("forge said no"));
		lite.getReview.mockReturnValue(new Promise(() => {}));
		render({ request: ["alice"] });

		await act(async () => {
			click("request");
			await vi.waitFor(() => expect(lite.getReview).toHaveBeenCalled());
		});
		expect(queryClient.getQueryData(reviewKey)).toEqual(reviewWith([bob]));
		await act(() => vi.waitFor(() => expect(reviewers()).toBe("bob:1")));
	});

	it("hides a withdrawn reviewer before the forge answers", async () => {
		lite.withdrawReviewRequest.mockReturnValue(new Promise(() => {}));
		render({ withdraw: ["bob"] });

		await act(async () => {
			click("withdraw");
			await vi.waitFor(() => expect(reviewers()).toBe(""));
		});
		expect(lite.withdrawReviewRequest.mock.calls[0]?.[0]).toEqual({
			projectId,
			reviewId,
			logins: ["bob"],
		});
	});
});

/** @vitest-environment jsdom */
import "fake-indexeddb/auto";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { reviewStateQueryOptions } from "./review-state.ts";
const client = new QueryClient();
import { describe, expect, it, vi } from "vitest";
import type { ForgeReview } from "@gitbutler/but-sdk";
import { coalesceInboxEntries, useReviewActivityInbox } from "./review-notifications.ts";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import {
	currentForgeLoginQueryOptions,
	forgeInfoOptions,
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	listReviewsQueryOptions,
} from "./api/queries.ts";
import type { ReviewState } from "./review-state.ts";
import { addInboxEntries } from "./review-inbox.ts";

const review = (overrides: Partial<ForgeReview> = {}): ForgeReview => ({
	htmlUrl: "https://forge.example/pr/7",
	number: 7,
	title: "A change",
	body: null,
	author: null,
	labels: [],
	draft: false,
	sourceBranch: "feature",
	targetBranch: "main",
	sha: "abc",
	integrationCommitShas: [],
	createdAt: "2026-08-28T09:00:00Z",
	modifiedAt: "2026-08-28T10:00:00Z",
	mergedAt: null,
	closedAt: null,
	repositorySshUrl: null,
	repositoryHttpsUrl: null,
	repoOwner: null,
	headRepoIsFork: false,
	reviewers: [],
	autoMergeEnabled: false,
	unitSymbol: "#",
	lastSyncAt: "2026-08-28T10:00:00Z",
	...overrides,
});

describe("coalesceInboxEntries", () => {
	it("files human and agent comments from one poll separately, even at the same timestamp", async () => {
		const atMs = Date.parse("2026-08-28T11:00:00Z");
		const items = coalesceInboxEntries(
			review(),
			[
				{ kind: "comment", id: 1, author: "alice", authorIsBot: false, body: "Human note", atMs },
				{ kind: "comment", id: 2, author: "copilot", authorIsBot: true, body: "Agent note", atMs },
				{
					kind: "comment",
					id: 3,
					author: "alice",
					authorIsBot: false,
					body: "Earlier human note",
					atMs: atMs - 1000,
				},
			],
			null,
		);
		expect(items).toHaveLength(2);
		expect(items[0]).toMatchObject({
			id: "7:comment:2026-08-28T11:00:00.000Z",
			author: "alice",
			authorIsBot: false,
			count: 2,
			commentId: 1,
			snippet: "Human note",
		});
		expect(items[1]).toMatchObject({
			id: "7:comment:2026-08-28T11:00:00.000Z:bot",
			author: "copilot",
			authorIsBot: true,
			count: 1,
			commentId: 2,
			snippet: "Agent note",
		});
		const projectId = "coalescing-test";
		expect(await addInboxEntries(client, projectId, items)).toHaveLength(2);
		expect(client.getQueryData(reviewStateQueryOptions(projectId).queryKey)?.inbox).toHaveLength(2);
	});
});

it("retains the first listing while storage hydrates, so the next poll remains new", async () => {
	globalThis.IS_REACT_ACT_ENVIRONMENT = true;
	const projectId = "hydrating-detector";
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false, staleTime: Infinity } },
	});
	const ready = Promise.withResolvers<ReviewState>();
	const hydration = client.fetchQuery({
		...reviewStateQueryOptions(projectId),
		queryFn: () => ready.promise,
	});
	const listingKey = listReviewsQueryOptions({ projectId, cacheConfig: "noCache" }).queryKey;
	client.setQueryData(guiSettingsQueryOptions.queryKey, {
		version: 1,
		prNotifications: "loud",
		desktopNotifications: false,
	});
	client.setQueryData(currentForgeLoginQueryOptions(projectId).queryKey, "me");
	client.setQueryData(listingKey, [review()]);
	vi.stubGlobal("lite", {
		forgeInfo: async () => ({ capabilities: { prService: true, reviewComments: true } }),
		headInfo: async () => ({ stacks: [] }),
		listReviewComments: async () => [
			{
				id: 1,
				body: "Please look @me",
				createdAt: "2026-08-28T10:30:00Z",
				author: { login: "alice", isBot: false },
				reactions: [],
			},
		],
		listReviewSubmissions: async () => [],
		listReviewThreads: async () => [],
		listReviewTimelineEvents: async () => [],
		onNotificationClick: () => () => {},
	});
	await client.fetchQuery(forgeInfoOptions(projectId));
	await client.fetchQuery(headInfoQueryOptions(projectId));
	const root = createRoot(document.createElement("div"));
	const Observe = () => {
		useReviewActivityInbox(projectId);
		return null;
	};
	try {
		await act(async () => {
			root.render(createElement(QueryClientProvider, { client }, createElement(Observe)));
		});
		await act(async () => {
			client.setQueryData(listingKey, [review({ modifiedAt: "2026-08-28T11:00:00Z" })]);
			await new Promise((resolve) => setTimeout(resolve, 0));
		});
		await act(async () => {
			ready.resolve({ marks: {}, unseen: {}, inbox: [] });
			await hydration;
		});
		await vi.waitFor(() =>
			expect(client.getQueryData(reviewStateQueryOptions(projectId).queryKey)?.inbox).toMatchObject(
				[{ kind: "mention", review: 7 }],
			),
		);
	} finally {
		await act(async () => root.unmount());
		client.clear();
		vi.unstubAllGlobals();
	}
});

it("waits for fresh marks when enabling the detector with an invalidated cache", async () => {
	globalThis.IS_REACT_ACT_ENVIRONMENT = true;
	const projectId = "stale-detector";
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false, staleTime: Infinity } },
	});
	const options = reviewStateQueryOptions(projectId);
	client.setQueryData(options.queryKey, {
		marks: { 7: "2026-08-28T09:00:00Z" },
		unseen: {},
		inbox: [],
	});
	client.setQueryData(guiSettingsQueryOptions.queryKey, {
		version: 1,
		prNotifications: "off",
		desktopNotifications: false,
	});
	client.setQueryData(currentForgeLoginQueryOptions(projectId).queryKey, "me");
	client.setQueryData(listReviewsQueryOptions({ projectId, cacheConfig: "noCache" }).queryKey, [
		review(),
	]);
	const comments = vi.fn(async () => [
		{
			id: 1,
			body: "@me already read this",
			createdAt: "2026-08-28T09:30:00Z",
			author: { login: "alice", isBot: false },
			reactions: [],
		},
	]);
	vi.stubGlobal("lite", {
		forgeInfo: async () => ({ capabilities: { prService: true, reviewComments: true } }),
		headInfo: async () => ({ stacks: [] }),
		listReviewComments: comments,
		listReviewSubmissions: async () => [],
		listReviewThreads: async () => [],
		listReviewTimelineEvents: async () => [],
		onNotificationClick: () => () => {},
	});
	await client.fetchQuery(forgeInfoOptions(projectId));
	await client.fetchQuery(headInfoQueryOptions(projectId));
	const root = createRoot(document.createElement("div"));
	const Observe = () => {
		useReviewActivityInbox(projectId);
		return null;
	};
	const ready = Promise.withResolvers<ReviewState>();
	try {
		await act(async () => {
			root.render(createElement(QueryClientProvider, { client }, createElement(Observe)));
		});
		await client.invalidateQueries({ queryKey: options.queryKey });
		const refresh = client.fetchQuery({ ...options, staleTime: 0, queryFn: () => ready.promise });
		await act(async () => {
			client.setQueryData(guiSettingsQueryOptions.queryKey, {
				version: 1,
				prNotifications: "loud",
				desktopNotifications: false,
			});
			await new Promise((resolve) => setTimeout(resolve, 0));
		});
		expect(comments).not.toHaveBeenCalled();
		await act(async () => {
			ready.resolve({ marks: { 7: "2026-08-28T10:00:00Z" }, unseen: {}, inbox: [] });
			await refresh;
		});
		expect(comments).not.toHaveBeenCalled();
		expect(client.getQueryData(options.queryKey)?.inbox).toEqual([]);
	} finally {
		ready.resolve({ marks: {}, unseen: {}, inbox: [] });
		await act(async () => root.unmount());
		client.clear();
		vi.unstubAllGlobals();
	}
});

import WorkspaceUpstreamModalHarness from "./WorkspaceUpstreamModalHarness.svelte";
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, test, vi } from "vitest";
import type { BaseBranch, Commit, RefInfo, RemoteCommit, Segment, Stack } from "@gitbutler/but-sdk";

function makeCommit(id: string, options?: { hasConflicts?: boolean; integrated?: boolean }): Commit {
	return {
		id,
		parentIds: [],
		message: id,
		hasConflicts: options?.hasConflicts ?? false,
		state: options?.integrated ? { type: "Integrated" } : { type: "LocalOnly" },
		createdAt: 0,
		author: { name: "Test", email: "test@example.com", gravatarUrl: "" },
		changeId: id,
		gerritReviewUrl: null,
	};
}

function makeSegment(name: string, commitIds: string[], options?: { hasConflicts?: boolean; integrated?: boolean }): Segment {
	return {
		refName: {
			fullNameBytes: Array.from(new TextEncoder().encode(`refs/heads/${name}`)),
			displayName: name,
		},
		remoteTrackingRefName: null,
		commits: commitIds.map((id) =>
			makeCommit(id, {
				hasConflicts: options?.hasConflicts,
				integrated: options?.integrated,
			}),
		),
		commitsOnRemote: [],
		commitsOutside: null,
		metadata: null,
		isEntrypoint: false,
		pushStatus: "nothingToPush",
		base: null,
	};
}

function makeStack(id: string | null, segments: Segment[]): Stack {
	return {
		id,
		base: null,
		segments,
	};
}

function makeHeadInfo(stacks: Stack[]): RefInfo {
	return {
		workspaceRef: null,
		stacks,
		target: null,
		isManagedRef: true,
		isManagedCommit: true,
		isEntrypoint: true,
	};
}

function makeBaseBranch(): BaseBranch {
	const upstreamCommit: RemoteCommit = {
		id: "upstream-1",
		description: "Upstream change",
		createdAt: 0,
		author: { name: "Remote", email: "remote@example.com", gravatarUrl: "" },
		changeId: null,
		parentIds: [],
		conflicted: false,
	};

	return {
		branchName: "refs/heads/main",
		remoteName: "origin",
		remoteUrl: "https://example.com/repo.git",
		pushRemoteName: "origin",
		pushRemoteUrl: "https://example.com/repo.git",
		baseSha: "base-sha",
		currentSha: "current-sha",
		behind: 1,
		upstreamCommits: [upstreamCommit],
		recentCommits: [upstreamCommit],
		lastFetchedMs: null,
		conflicted: false,
		diverged: false,
		divergedAhead: [],
		divergedBehind: [],
		shortName: "main",
	};
}

describe("IntegrateUpstreamWorkspaceModal", () => {
	test("shows only Rebase and Merge actions", async () => {
		render(WorkspaceUpstreamModalHarness, {
			props: {
				base: makeBaseBranch(),
				currentHeadInfo: makeHeadInfo([makeStack("stack-1", [makeSegment("feature", ["c2", "c1"])])]),
				previewHeadInfo: makeHeadInfo([makeStack("stack-1", [makeSegment("feature", ["c2p", "c1p"])])]),
			},
		});

		await screen.findByTestId("integrate-upstream-commits-modal");

		expect(screen.getByTestId("integrate-upstream-action-button")).toBeInTheDocument();
		expect(screen.queryByText("Conflicting uncommitted files")).not.toBeInTheDocument();
		expect(screen.queryByText("Stash")).not.toBeInTheDocument();
	});

	test("disables Update workspace when preview fails", async () => {
		const onIntegrate = vi.fn(async () => undefined);
		render(WorkspaceUpstreamModalHarness, {
			props: {
				base: makeBaseBranch(),
				currentHeadInfo: makeHeadInfo([makeStack("stack-1", [makeSegment("feature", ["c2", "c1"])])]),
				previewError: "Preview failed",
				onIntegrate,
			},
		});

		await waitFor(() => expect(screen.getByText((content) => content.includes("Preview failed"))).toBeInTheDocument());
		expect(screen.getByTestId("integrate-upstream-action-button")).toBeDisabled();

		const form = screen.getByTestId("integrate-upstream-commits-modal").querySelector("form");
		expect(form).not.toBeNull();
		await fireEvent.submit(form!);
		expect(onIntegrate).not.toHaveBeenCalled();
	});

	test("marks only newly conflicted branches", async () => {
		render(WorkspaceUpstreamModalHarness, {
			props: {
				base: makeBaseBranch(),
				currentHeadInfo: makeHeadInfo([
					makeStack("stack-1", [makeSegment("feature", ["c2", "c1"])]),
					makeStack("stack-2", [makeSegment("already-conflicted", ["x2", "x1"], { hasConflicts: true })]),
				]),
				previewHeadInfo: makeHeadInfo([
					makeStack("stack-1", [makeSegment("feature", ["c2p", "c1p"], { hasConflicts: true })]),
					makeStack("stack-2", [makeSegment("already-conflicted", ["x2p", "x1p"], { hasConflicts: true })]),
				]),
			},
		});

		await screen.findAllByTestId("integrate-upstream-series-row");
		expect(screen.getAllByText("Conflicted")).toHaveLength(1);
	});
});

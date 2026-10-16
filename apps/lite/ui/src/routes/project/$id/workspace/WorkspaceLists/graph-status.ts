import { commitIsDiverged } from "#ui/commit.ts";
import type { GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import type { Commit, PushStatus } from "@gitbutler/but-sdk";

export const segmentPushStatusToGraphSegmentStatus = (
	pushStatus: PushStatus,
): GraphSegmentStatus => {
	switch (pushStatus) {
		case "nothingToPush":
			return "LocalAndRemote";
		case "unpushedCommits":
		case "completelyUnpushed":
			return "LocalOnly";
		case "unpushedCommitsRequiringForce":
			return "Diverged";
		case "integrated":
			return "Integrated";
	}
};

export const commitGraphStatus = (commit: Commit): GraphSegmentStatus =>
	commitIsDiverged(commit) ? "Diverged" : commit.state.type;

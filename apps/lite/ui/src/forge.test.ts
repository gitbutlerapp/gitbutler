import { assert } from "#ui/assert.ts";
import { describe, expect, it } from "vitest";
import type { ForgeInfo } from "@gitbutler/but-sdk";
import { forgeAuthFailure, forgeDestination, isCloudForge } from "./forge.ts";

const info = { name: "github", baseUrl: "https://github.com/acme/repo" } as ForgeInfo;
describe("forge", () => {
	it("uses the project's forge and the review's host", () => {
		expect(forgeDestination(info, "https://git.example.com/acme/repo/-/merge_requests/5")).toEqual({
			name: "github",
			label: "GitHub",
			host: "git.example.com",
		});
	});
	it("does not identify an enterprise host as a cloud forge", () => {
		const destination = assert(forgeDestination(info, "https://git.example.com/acme/repo/pull/5"));
		expect(isCloudForge(destination)).toBe(false);
	});
	it("recognizes GitHub cloud", () => {
		expect(isCloudForge(assert(forgeDestination(info)))).toBe(true);
	});
	it("recognizes Bitbucket cloud", () => {
		const destination = assert(
			forgeDestination(
				{ ...info, name: "bitbucket" },
				"https://bitbucket.org/acme/repo/pull-requests/5",
			),
		);
		expect(isCloudForge(destination)).toBe(true);
	});
	it("distinguishes backend login failures from network and permission errors", () => {
		expect(
			forgeAuthFailure(
				new Error(
					"Error invoking remote method 'listReviews': Error: GitHub authentication failed.",
				),
			),
		).toBe("rejected");
		expect(
			forgeAuthFailure(
				new Error(
					"Not authenticated with GitLab. Connect your account under Settings → Integrations.",
				),
			),
		).toBe("missing");
		expect(forgeAuthFailure(new Error("Unable to connect to GitHub."))).toBeNull();
		expect(
			forgeAuthFailure(
				new Error("A GitHub organization has restricted access for the GitButler OAuth app."),
			),
		).toBeNull();
	});
});

/**
 * Retrieve the host platform in a
 * best-effort way w/ normalized output.
 */

export type OS = "macOS" | "Windows" | "Linux" | "unknown";

export function getOS(): OS {
	if (typeof navigator === "undefined") return "unknown";

	const userAgent = navigator.userAgent.toLowerCase();

	if (userAgent.includes("mac")) return "macOS";
	if (userAgent.includes("win")) return "Windows";
	if (userAgent.includes("linux")) return "Linux";

	return "unknown";
}

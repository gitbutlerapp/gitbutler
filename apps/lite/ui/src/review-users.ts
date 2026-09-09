import type { ForgeReviewUser } from "@gitbutler/but-sdk";

/**
 * Whether the user is an agent of any kind — Copilot, CI, a review bot.
 * The forge's own flag when it survives the trip, else the `[bot]` login
 * suffix every GitHub App carries.
 */
export const isAgent = (user: ForgeReviewUser): boolean =>
	user.isBot || user.login.endsWith("[bot]");

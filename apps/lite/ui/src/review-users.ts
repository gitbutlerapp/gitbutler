import type { ForgeReviewUser } from "@gitbutler/but-sdk";

/**
 * Whether the user is an agent of any kind — Copilot, CI, a review bot.
 * The forge's own flag when it survives the trip, else the `[bot]` login
 * suffix every GitHub App carries.
 */
export const isAgent = (user: ForgeReviewUser): boolean =>
	user.isBot || user.login.endsWith("[bot]");

/**
 * Whether two logins name the same account. GitHub spells a bot's login with
 * the `[bot]` suffix in REST and without it in GraphQL, and both reach the app.
 */
export const sameLogin = (a: string, b: string): boolean => bare(a) === bare(b);

const bare = (login: string): string => login.replace(/\[bot\]$/, "");

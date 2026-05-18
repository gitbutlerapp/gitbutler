import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "../../..");

export const BUT = process.env.BUT_BIN || path.join(ROOT, "target", "debug", "but");

export const GITHUB_TEST_TOKEN = process.env.GITHUB_TEST_TOKEN || "";
export const GITLAB_TEST_TOKEN = process.env.GITLAB_TEST_TOKEN || "";

export const GITHUB_TEST_REPO = process.env.GITHUB_TEST_REPO || "";
export const GITLAB_TEST_REPO = process.env.GITLAB_TEST_REPO || "";

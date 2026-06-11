// To be removed when this PR lands: https://github.com/gitbutlerapp/gitbutler/pull/14191
export const randomBranchRef = (): string => `refs/heads/${crypto.randomUUID().slice(0, 8)}`;

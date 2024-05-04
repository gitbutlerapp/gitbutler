import type { Project } from '$lib/backend/projects'
import type { BaseBranch, RemoteCommit, Author } from '$lib/vbranches/types'
import type { User } from '$lib/stores/user'

export type Constructor<T = any> = new (...args: any[]) => T;
export type Class<T = any> = InstanceType<Constructor<T>>;

export const project: Class<Project> = {
    id: "abc123",
    title: 'My Project',
    description: "A project description",
    path: "/Users/user/project",
    api: {
        name: 'github',
        description: 'description',
        repository_id: 'abc123',
        git_url: 'https://github.com/user/project',
        created_at: '2021-07-01T00:00:00Z',
        updated_at: '2021-07-01T00:01:00Z',
        sync: false
    },
    preferred_key: {
        local: { private_key_path: '/Users/.ssh/id_rsa' },
    },
    ok_with_force_push: true,
    omit_certificate_check: true,
    use_diff_context: true,
    vscodePath: '/Users/user/project'
}

export const author: Class<Author> = {
    name: 'John Snow',
    email: 'user@company.com',
    gravatarUrl: new URL('https://gravatar.com/abc123'),
    isBot: false
}

// @ts-expect-error
export const remoteCommit: Class<RemoteCommit> = {
    id: 'abc123',
    author,
    description: 'A commit message',
    createdAt: new Date(),
    isLocal: false,
}

// @ts-expect-error
export const baseBranch: Class<BaseBranch> = {
    branchName: 'main',
    remoteName: 'origin',
    remoteUrl: 'ssh://github.com/user/project.git',
    baseSha: '90c225edcc74b31718a9cd8963c1bc89c17d8864',
    currentSha: '90c225edcc74b31718a9cd8963c1bc89c17d8864',
    behind: 0,
    upstreamCommits: [],
    recentCommits: [remoteCommit],
    lastFetchedMs: 1714843209991,
    lastFetched: new Date(),
    repoBaseUrl: 'https://github.com/user/project',
    shortName: 'main',
    branchUrl: () => 'https://github.com/user/project',
    commitUrl: () => 'https://github.com/user/project',
    isGitlab: false,
    isBitBucket: false
}

export const user: Class<User> = {
    id: 123,
    name: "John Snow",
    given_name: "John",
    family_name: "Snow",
    email: "user@comapny.com",
    picture: "",
    locale: "en_EN",
    created_at: "2021-07-01T00:00:00Z",
    updated_at: "2021-07-01T00:01:00Z",
    access_token: "abc123",
    role: "ADMIN",
    supporter: true,
    github_access_token: undefined,
    github_username: undefined
}

import { dev } from "$app/environment";

const apiUrl = dev
    ? new URL("https://test.app.gitbutler.com/api/")
    : new URL("https://app.gitbutler.com/api/");

const getUrl = (path: string) => new URL(path, apiUrl).toString();

export type LoginToken = {
    token: string;
    expires: string;
    url: string;
};

export type User = {
    id: number;
    name: string;
    email: string;
    picture: string;
    locale: string;
    created_at: string;
    updated_at: string;
    access_token: string;
};

export type Project = {
    name: string;
    description: string | null;
    repository_id: string;
    git_url: string;
    created_at: string;
    updated_at: string;
};

const parseResponseJSON = async (response: Response) => {
    if (response.status === 204 || response.status === 205) {
        return null;
    } else if (response.status >= 400) {
        throw new Error(
            `HTTP Error ${response.statusText}: ${await response.text()}`
        );
    } else {
        return await response.json();
    }
};

export default (
    { fetch }: { fetch: typeof window.fetch } = { fetch: window.fetch }
) => ({
    login: {
        token: {
            create: (params: {} = {}): Promise<LoginToken> =>
                fetch(getUrl("login/token.json"), {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json",
                    },
                    body: JSON.stringify(params),
                })
                    .then(parseResponseJSON)
                    .then((token) => {
                        const url = new URL(token.url);
                        url.host = apiUrl.host;
                        return {
                            ...token,
                            url: url.toString(),
                        };
                    }),
        },
        user: {
            get: (token: string): Promise<User> =>
                fetch(getUrl(`login/user/${token}.json`), {
                    method: "GET",
                }).then(parseResponseJSON),
        },
    },
    projects: {
        create: (
            token: string,
            params: { name: string; uid?: string }
        ): Promise<Project> =>
            fetch(getUrl("projects.json"), {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                    "X-Auth-Token": token,
                },
                body: JSON.stringify(params),
            }).then(parseResponseJSON),
        list: (token: string): Promise<Project[]> =>
            fetch(getUrl("projects.json"), {
                method: "GET",
                headers: {
                    "X-Auth-Token": token,
                },
            }).then(parseResponseJSON),
        get: (token: string, repositoryId: string): Promise<Project> =>
            fetch(getUrl(`projects/${repositoryId}.json`), {
                method: "GET",
                headers: {
                    "X-Auth-Token": token,
                },
            }).then(parseResponseJSON),
        delete: (token: string, repositoryId: string): Promise<void> =>
            fetch(getUrl(`projects/${repositoryId}.json`), {
                method: "DELETE",
                headers: {
                    "X-Auth-Token": token,
                },
            }).then(parseResponseJSON),
    },
});

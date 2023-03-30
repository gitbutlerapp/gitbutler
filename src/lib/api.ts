import { PUBLIC_API_BASE_URL } from '$env/static/public';
import * as log from '$lib/log';
import { nanoid } from 'nanoid';

const apiUrl = new URL('/api/', new URL(PUBLIC_API_BASE_URL));

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
        throw new Error(`HTTP Error ${response.statusText}: ${await response.text()}`);
    } else {
        return await response.json();
    }
};

type FetchMiddleware = (f: typeof fetch) => typeof fetch;

const fetchWith = (fetch: typeof window.fetch, ...middlewares: FetchMiddleware[]) =>
    middlewares.reduce((f, middleware) => middleware(f), fetch);

const withRequestId: FetchMiddleware = (fetch) => async (url, options) => {
    const requestId = nanoid();
    if (!options) options = {};
    options.headers = {
        ...options?.headers,
        'X-Request-Id': requestId
    };
    return fetch(url, options);
};

const withLog: FetchMiddleware = (fetch) => async (url, options) => {
    log.info('fetch', url, options);
    try {
        const resp = await fetch(url, options);
        log.info(resp);
        return resp;
    } catch (e: any) {
        log.error('fetch', e);
        throw e;
    }
};

export default (
    { fetch: realFetch }: { fetch: typeof window.fetch } = {
        fetch: window.fetch
    }
) => {
    const fetch = fetchWith(realFetch, withRequestId, withLog);
    return {
        login: {
            token: {
                create: (): Promise<LoginToken> =>
                    fetch(getUrl('login/token.json'), {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json'
                        },
                        body: JSON.stringify({})
                    })
                        .then(parseResponseJSON)
                        .then((token) => {
                            const url = new URL(token.url);
                            url.host = apiUrl.host;
                            return {
                                ...token,
                                url: url.toString()
                            };
                        })
            },
            user: {
                get: (token: string): Promise<User> =>
                    fetch(getUrl(`login/user/${token}.json`), {
                        method: 'GET'
                    }).then(parseResponseJSON)
            }
        },
        user: {
            get: async (token: string): Promise<User> =>
                fetch(getUrl(`user.json`), {
                    method: 'GET',
                    headers: {
                        'X-Auth-Token': token
                    }
                }).then(parseResponseJSON),
            update: async (token: string, params: { name?: string; picture?: File }) => {
                const formData = new FormData();
                if (params.name) {
                    formData.append('name', params.name);
                }
                if (params.picture) {
                    formData.append('avatar', params.picture);
                }
                return fetch(getUrl(`user.json`), {
                    method: 'PUT',
                    headers: {
                        'X-Auth-Token': token
                    },
                    body: formData
                }).then(parseResponseJSON);
            }
        },
        summarize: {
            commit: (
                token: string,
                params: { diff: string; uid?: string }
            ): Promise<{ message: string }> =>
                fetch(getUrl('summarize/commit.json'), {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                        'X-Auth-Token': token
                    },
                    body: JSON.stringify(params)
                }).then(parseResponseJSON)
        },
        projects: {
            create: (token: string, params: { name: string; uid?: string }): Promise<Project> =>
                fetch(getUrl('projects.json'), {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                        'X-Auth-Token': token
                    },
                    body: JSON.stringify(params)
                }).then(parseResponseJSON),
            update: (
                token: string,
                repositoryId: string,
                params: { name: string; description?: string }
            ): Promise<Project> =>
                fetch(getUrl(`projects/${repositoryId}.json`), {
                    method: 'PUT',
                    headers: {
                        'Content-Type': 'application/json',
                        'X-Auth-Token': token
                    },
                    body: JSON.stringify(params)
                }).then(parseResponseJSON),
            list: (token: string): Promise<Project[]> =>
                fetch(getUrl('projects.json'), {
                    method: 'GET',
                    headers: {
                        'X-Auth-Token': token
                    }
                }).then(parseResponseJSON),
            get: (token: string, repositoryId: string): Promise<Project> =>
                fetch(getUrl(`projects/${repositoryId}.json`), {
                    method: 'GET',
                    headers: {
                        'X-Auth-Token': token
                    }
                }).then(parseResponseJSON),
            delete: (token: string, repositoryId: string): Promise<void> =>
                fetch(getUrl(`projects/${repositoryId}.json`), {
                    method: 'DELETE',
                    headers: {
                        'X-Auth-Token': token
                    }
                }).then(parseResponseJSON)
        }
    };
};

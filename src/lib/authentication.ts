import { dev } from '$app/environment'
import type { Project } from '$lib/projects';
import type { User } from '$lib/user';

const apiUrl = dev ? new URL('https://test.app.gitbutler.com/api/') : new URL('https://app.gitbutler.com/api/');

const getUrl = (path: string) => new URL(path, apiUrl).toString();

export type LoginToken = {
    token: string,
    expires: string,
    url: string
}


const parseJSON = async (response: Response) => {
    if (response.status === 204 || response.status === 205) {
        return null;
    }
    if (response.status >= 400) {
        throw new Error(`HTTP Error ${response.statusText}: ${await response.text()}`);
    }
    return await response.json();
}

export default ({ fetch }: { fetch: typeof window.fetch } = { fetch: window.fetch }) => ({
    login: {
        token: {
            create: (params: {} = {}): Promise<LoginToken> => fetch(getUrl('login/token.json'), {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(params),
            }).then(parseJSON).then(token => {
                const url = new URL(token.url);
                url.host = apiUrl.host;
                return {
                    ...token,
                    url: url.toString(),
                }

            }),
        },
        user: {
            get: (token: string): Promise<User> => fetch(getUrl(`login/user/${token}.json`), {
                method: 'GET',
            }).then(parseJSON),
        }
    },
    project: {
        get: (repoId: string): Promise<Project> => fetch(getUrl(`projects/${repoId}.json`), {
            method: 'GET',
        }).then(parseJSON),
        create: (token: string, params: {} = {}): Promise<Project> => fetch(getUrl(`projects.json`), {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'X-Auth-Token': token
            },
            body: JSON.stringify(params),
        }).then(parseJSON),
    }
})

import type { Toast } from '$lib/notifications/toasts';

/**
 * Example error responses
 * ```
 * {
 *   "name": "HttpError",
 *   "request": {
 *      "body": "{\"head\":\"branch1\",\"base\":\"main\",\"title\":\"Some title\",\"body\":\"\",\"draft\":false}",
 *      "headers": {
 *        "accept": "application/vnd.github.v3+json",
 *        "authorization": "token [REDACTED]",
 *        "content-type": "application/json; charset=utf-8",
 *        "user-agent": "GitButler Client octokit-rest.js/20.0.2 octokit-core.js/5.0.1 Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)"
 *      },
 *      "method": "POST",
 *      "request": {
 *        "hook": {}
 *      "url": "https://api.github.com/repos/someuser/somerepo/pulls"
 *     },
 *     "response": {
 *       "data": {
 *         "documentation_url": "https://docs.github.com/rest/pulls/pulls#create-a-pull-request",
 *         "message": "Although you appear to have the correct authorization credentials, the organization has enabled OAuth App access restrictions, meaning that data access to third-parties is limited. For more information on these restrictions, including how to enable this app, visit https://docs.github.com/articles/restricting-access-to-your-organization-s-data/"
 *       },
 *       "headers": {
 *         "content-type": "application/json; charset=utf-8",
 *         "x-accepted-oauth-scopes": "",
 *         "x-github-media-type": "github.v3; format=json",
 *         "x-github-request-id": "F93F:2A6A69:1CEAF8:1D2E00:65D486FC",
 *         "x-oauth-scopes": "repo",
 *         "x-ratelimit-limit": "5000",
 *         "x-ratelimit-remaining": "4968",
 *         "x-ratelimit-reset": "1708427744",
 *         "x-ratelimit-resource": "core",
 *         "x-ratelimit-used": "32"
 *       },
 *       "status": 403,
 *       "url": "https://api.github.com/repos/someuser/somerepo/pulls"
 *     },
 *   "status": 403
 * }
 *
 * {
 *   name: 'HttpError',
 *   request: {
 *     body: '{"head":"branch2","base":"main","title":"some title","body":"","draft":false}',
 *     headers: {
 *       accept: 'application/vnd.github.v3+json',
 *       authorization: 'token [REDACTED]',
 *       'content-type': 'application/json; charset=utf-8',
 *       'user-agent': 'GitButler Client octokit-rest.js/20.0.2 octokit-core.js/5.0.1 Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)'
 *     },
 *     method: 'POST',
 *     request: {
 *         hook: {}
 *     },
 *     url: 'https://api.github.com/repos/someuser/somerepo/api/pulls'
 *   },
 *   response: {
 *     data: {
 *       documentation_url:
 *           'https://docs.github.com/rest/pulls/pulls#create-a-pull-request',
 *       errors: [
 *         {
 *            code: 'custom',
 *            message: 'A pull request already exists for someuser:somebranch.',
 *            resource: 'PullRequest'
 *         }
 *       ],
 *       message: 'Validation Failed'
 *     },
 *     headers: {
 *       'content-length': '266',
 *       'content-type': 'application/json; charset=utf-8',
 *       'x-accepted-oauth-scopes': '',
 *       'x-github-media-type': 'github.v3; format=json',
 *       'x-github-request-id': 'C233:72D21:6493:6C61:65D366B1',
 *       'x-oauth-scopes': 'repo',
 *       'x-ratelimit-limit': '5000',
 *       'x-ratelimit-remaining': '4994',
 *       'x-ratelimit-reset': '1708356743',
 *       'x-ratelimit-resource': 'core',
 *       'x-ratelimit-used': '6'
 *     },
 *     status: 422,
 *     url: 'https://api.github.com/repos/someuser/somerepo/pulls'
 *   },
 *   status: 422
 * }
 * ```
 *
 * {
 *   "name": "HttpError",
 *   "request": {
 *     "body": "{\"head\":\"Update-vscode-colors\",\"base\":\"C1-393-docker-implementation\",\"title\":\"Update vscode colors\",\"body\":\"\",\"draft\":false}",
 *     "headers": {
 *       "accept": "application/vnd.github.v3+json",
 *       "authorization": "token [REDACTED]",
 *       "content-type": "application/json; charset=utf-8",
 *       "user-agent": "GitButler Client octokit-rest.js/20.0.2 octokit-core.js/5.0.1 Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)"
 *     },
 *     "method": "POST",
 *     "request": {
 *       "hook": {}
 *     },
 *     "url": "https://api.github.com/repos/c1-ab/c1-backend/pulls"
 *   },
 *   "response": {
 *     "data": {
 *       "documentation_url": "https://docs.github.com/rest/pulls/pulls#create-a-pull-request",
 *       "errors": [
 *         {
 *           "code": "invalid",
 *           "field": "base",
 *           "resource": "PullRequest"
 *         }
 *       ],
 *       "message": "Validation Failed"
 *     },
 *     "headers": {
 *       "content-length": "186",
 *       "content-type": "application/json; charset=utf-8",
 *       "x-accepted-oauth-scopes": "",
 *       "x-github-media-type": "github.v3; format=json",
 *       "x-github-request-id": "E5EE:F1F0:6880D:6984F:65D74AC3",
 *       "x-oauth-scopes": "repo",
 *       "x-ratelimit-limit": "15000",
 *       "x-ratelimit-remaining": "14950",
 *       "x-ratelimit-reset": "1708609120",
 *       "x-ratelimit-resource": "core",
 *       "x-ratelimit-used": "50"
 *     },
 *     "status": 422,
 *     "url": "https://api.github.com/repos/c1-ab/c1-backend/pulls"
 *   },
 *   "status": 422
 * }
 */
export function mapErrorToToast(err: any): Toast | undefined {
	// We expect an object to be thrown by octokit.
	if (typeof err !== 'object') return;

	const { status, response } = err;
	const { data } = response;
	const { message, errors } = data;

	// If this expectation isn't met we must be doing something wrong
	if (status === undefined || message === undefined) return;

	if (message.includes('Draft pull requests are not supported')) {
		return {
			title: 'Draft pull requests are not enabled',
			message: `
                It looks like draft pull requests are not enabled in your repository.

                Please see our [documentation](https://docs.gitbutler.com/)
                for additional help.
            `,
			error: message,
			style: 'error'
		};
	}

	if (message.includes('enabled OAuth App access restrictions')) {
		return {
			title: 'OAuth access restricted',
			message: `
                It looks like OAuth access has been restricted by your organization.

                Please see our [documentation](https://docs.gitbutler.com/)
                for additional help.
            `,
			error: message,
			style: 'error'
		};
	}

	if (message.includes('Validation Failed')) {
		let errorStrings = '';
		if (errors instanceof Array) {
			errorStrings = errors
				.map((err) => {
					if (err.message) return err.message;
					if (err.field && err.code) return `${err.field} ${err.code}`;
					return 'unknown validation error';
				})
				.join('\n');
		}
		return {
			title: 'GitHub validation failed',
			message: `
                It seems there was a problem validating the request.

                Please see our [documentation](https://docs.gitbutler.com/)
                for additional help.
            `,
			error: errorStrings,
			style: 'error'
		};
	}
}
